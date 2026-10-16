//! Per-provider polling interval that tunes itself.
//!
//! Smart mode works like congestion control: a rate-limit answer doubles the interval and remembers
//! a floor just above the interval that failed; successes then creep the interval back down but never
//! below that floor, and the floor itself is slowly forgiven. So the interval settles just above the
//! provider's real tolerance. On top of that the target follows activity: faster near the limit,
//! slower while usage does not change.

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);
/// Successful polls in a row before the interval is allowed to step down.
const STEP_DOWN_AFTER: u32 = 3;
/// Successful polls between floor forgiveness steps.
const FORGIVE_EVERY: u32 = 10;
/// Polls without a change in usage before the idle slowdown starts / deepens.
const IDLE_AFTER: u32 = 3;
const IDLE_DEEP_AFTER: u32 = 8;

/// Everything the tuner needs to know about the current configuration of one provider.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub smart: bool,
    /// User-chosen interval for this provider.
    pub base: Duration,
    /// Smart-mode bounds (already raised to the provider's own minimum).
    pub lo: Duration,
    pub hi: Duration,
    pub boost_near_limit: bool,
    pub slow_when_idle: bool,
    pub warn_pct: f32,
}

impl Params {
    fn clamp(&self, d: Duration) -> Duration {
        d.clamp(self.lo, self.hi.max(self.lo))
    }
}

/// What the interval is currently doing; shown in the settings and flyout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TuneState {
    Fixed,
    Steady,
    Boosted,
    Relaxed,
    RateLimited,
}

/// The part worth keeping between launches.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Learned {
    pub cur_secs: u64,
    pub floor_secs: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct Tuner {
    cur: Option<Duration>,
    floor: Option<Duration>,
    ok_streak: u32,
    unchanged: u32,
    last_pct: Option<f32>,
    /// Consecutive rate-limit answers.
    pub streak_429: u32,
    pub total_429: u32,
}

impl Tuner {
    pub fn from_learned(l: Learned) -> Self {
        Self {
            cur: (l.cur_secs > 0).then(|| Duration::from_secs(l.cur_secs)),
            floor: l.floor_secs.map(Duration::from_secs),
            ..Self::default()
        }
    }

    /// Only a learned floor is worth persisting; plain activity tuning is cheap to redo.
    pub fn learned(&self) -> Option<Learned> {
        self.floor.map(|f| Learned {
            cur_secs: self.cur.map_or(0, |c| c.as_secs()),
            floor_secs: Some(f.as_secs()),
        })
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn floor(&self) -> Option<Duration> {
        self.floor
    }

    /// The interval in effect right now.
    pub fn current(&self, p: &Params) -> Duration {
        if !p.smart {
            return p.base.max(p.lo);
        }
        p.clamp(self.cur.unwrap_or(p.base))
    }

    pub fn state(&self, p: &Params) -> TuneState {
        if !p.smart {
            return TuneState::Fixed;
        }
        let cur = self.current(p);
        if self.streak_429 > 0 || self.floor.is_some() {
            TuneState::RateLimited
        } else if cur < p.base {
            TuneState::Boosted
        } else if cur > p.base {
            TuneState::Relaxed
        } else {
            TuneState::Steady
        }
    }

    /// Where the interval would like to be, from activity alone.
    fn target(&self, p: &Params, pct: Option<f32>) -> Duration {
        let near = pct.is_some_and(|v| v >= p.warn_pct);
        let t = if p.boost_near_limit && near {
            p.base / 2
        } else if p.slow_when_idle && !near && self.unchanged >= IDLE_DEEP_AFTER {
            p.base * 3
        } else if p.slow_when_idle && !near && self.unchanged >= IDLE_AFTER {
            p.base * 3 / 2
        } else {
            p.base
        };
        p.clamp(t)
    }

    /// Records a good poll and returns the delay until the next one.
    pub fn on_success(&mut self, p: &Params, max_pct: Option<f32>) -> Duration {
        self.streak_429 = 0;
        self.ok_streak = self.ok_streak.saturating_add(1);
        match (self.last_pct, max_pct) {
            (Some(a), Some(b)) if (a - b).abs() < 0.05 => {
                self.unchanged = self.unchanged.saturating_add(1)
            }
            _ => self.unchanged = 0,
        }
        self.last_pct = max_pct;

        if !p.smart {
            return self.current(p);
        }
        let cur = self.cur.unwrap_or(p.base);
        let target = self.target(p, max_pct);
        let lowest = self.floor.unwrap_or(p.lo);
        let next = if target >= cur || self.floor.is_none() {
            target
        } else if self.ok_streak >= STEP_DOWN_AFTER {
            // Recovering from a rate limit: creep down instead of jumping back to the failing interval.
            target.max(cur * 9 / 10)
        } else {
            cur
        };
        self.cur = Some(p.clamp(next.max(lowest)));

        // Slowly forget old failures so one bad hour does not slow polling forever.
        if self.ok_streak.is_multiple_of(FORGIVE_EVERY) {
            if let Some(f) = self.floor {
                let f = f * 9 / 10;
                // A floor at or below the base no longer constrains anything.
                self.floor = (f > p.base.max(p.lo)).then_some(f);
            }
        }
        self.current(p)
    }

    /// Records a rate-limit answer and returns the delay until the next poll.
    pub fn on_rate_limited(&mut self, p: &Params, retry_after: Option<Duration>) -> Duration {
        self.total_429 = self.total_429.saturating_add(1);
        self.streak_429 = self.streak_429.saturating_add(1);
        self.ok_streak = 0;
        let asked = retry_after.unwrap_or_default();

        if !p.smart {
            // Fixed mode keeps its interval but still obeys the provider.
            let backoff = p
                .base
                .saturating_mul(1u32 << self.streak_429.min(5))
                .min(MAX_BACKOFF);
            return backoff.max(asked);
        }
        let cur = self.cur.unwrap_or(p.base);
        let floor = p.clamp(cur * 12 / 10);
        self.floor = Some(self.floor.map_or(floor, |f| f.max(floor)));
        let next = p.clamp((cur * 2).max(floor).max(asked));
        self.cur = Some(next);
        next.max(asked)
    }

    /// Applies new settings without throwing away what was learned.
    pub fn retarget(&mut self, p: &Params) {
        if let Some(c) = self.cur {
            self.cur = Some(p.clamp(c.max(self.floor.unwrap_or(p.lo))));
        }
        if let Some(f) = self.floor {
            self.floor = (f > p.lo).then_some(f.min(p.hi));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            smart: true,
            base: Duration::from_secs(180),
            lo: Duration::from_secs(60),
            hi: Duration::from_secs(1800),
            boost_near_limit: true,
            slow_when_idle: true,
            warn_pct: 80.0,
        }
    }

    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn rate_limit_doubles_and_sets_floor() {
        let p = params();
        let mut t = Tuner::default();
        assert_eq!(t.on_rate_limited(&p, None), S(360));
        assert_eq!(
            t.floor(),
            Some(S(216)),
            "floor is 20% above the interval that failed"
        );
        assert_eq!(t.state(&p), TuneState::RateLimited);
    }

    #[test]
    fn retry_after_is_obeyed_and_bounded() {
        let p = params();
        let mut t = Tuner::default();
        assert_eq!(t.on_rate_limited(&p, Some(S(900))), S(900));
        let mut t = Tuner::default();
        assert_eq!(
            t.on_rate_limited(&p, Some(S(99_999))),
            S(99_999),
            "next poll waits as asked"
        );
        assert!(
            t.current(&p) <= p.hi,
            "but the stored interval stays within bounds"
        );
    }

    #[test]
    fn steps_down_but_never_below_floor_and_floor_is_forgiven() {
        let p = params();
        let mut t = Tuner::default();
        t.on_rate_limited(&p, None); // cur 360, floor 216
        let mut seen = Vec::new();
        for i in 0..40 {
            // Alternating usage so the idle slowdown does not kick in.
            seen.push(t.on_success(&p, Some(10.0 + (i % 2) as f32)));
        }
        assert!(seen[0] >= S(216), "never below the floor while it exists");
        // After many good polls the floor is forgiven and the interval returns to the base.
        assert_eq!(*seen.last().unwrap(), S(180));
        assert!(
            t.floor().is_none(),
            "floor eventually forgiven: {:?}",
            t.floor()
        );
    }

    #[test]
    fn repeated_limits_grow_to_the_cap() {
        let p = params();
        let mut t = Tuner::default();
        let mut last = S(0);
        for _ in 0..10 {
            last = t.on_rate_limited(&p, None);
        }
        assert_eq!(last, p.hi);
    }

    #[test]
    fn boosts_near_limit_and_relaxes_when_idle() {
        let p = params();
        let mut t = Tuner::default();
        assert_eq!(
            t.on_success(&p, Some(85.0)),
            S(90),
            "near the limit: poll twice as often"
        );
        let mut t = Tuner::default();
        let mut d = S(0);
        for _ in 0..4 {
            d = t.on_success(&p, Some(20.0));
        }
        assert_eq!(d, S(270), "usage unchanged for a while: 1.5x");
        for _ in 0..6 {
            d = t.on_success(&p, Some(20.0));
        }
        assert_eq!(d, S(540), "idle for long: 3x");
        assert_eq!(
            t.on_success(&p, Some(21.0)),
            S(180),
            "activity resumes: back to the base"
        );
    }

    #[test]
    fn fixed_mode_keeps_interval_and_backs_off_only_on_limits() {
        let mut p = params();
        p.smart = false;
        let mut t = Tuner::default();
        assert_eq!(t.on_success(&p, Some(90.0)), S(180));
        assert_eq!(t.on_rate_limited(&p, None), S(360));
        assert_eq!(t.on_rate_limited(&p, None), S(720));
        assert_eq!(t.on_success(&p, Some(90.0)), S(180));
        assert_eq!(t.state(&p), TuneState::Fixed);
    }

    #[test]
    fn learned_state_round_trips() {
        let p = params();
        let mut t = Tuner::default();
        t.on_rate_limited(&p, None);
        let l = t.learned().unwrap();
        let back = Tuner::from_learned(l);
        assert_eq!(back.current(&p), S(360));
        assert_eq!(back.floor(), Some(S(216)));
    }
}
