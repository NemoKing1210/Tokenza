use serde::Serialize;

/// Typed provider failure; the UI picks the card text and action from the kind.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("agent is not logged in")]
    NotLoggedIn,
    #[error("token expired, log in to the agent CLI again")]
    TokenExpired,
    /// Carries the provider's `Retry-After` when it sent one.
    #[error("rate limited by the provider")]
    RateLimited(Option<std::time::Duration>),
    /// Signed in, but the account has no active plan to report limits for.
    #[error("no active subscription")]
    NoSubscription,
    #[error("response schema changed: {0}")]
    SchemaChanged(String),
    #[error("offline: {0}")]
    Offline(String),
}

impl ProviderError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotLoggedIn => "NotLoggedIn",
            Self::TokenExpired => "TokenExpired",
            Self::RateLimited(_) => "RateLimited",
            Self::NoSubscription => "NoSubscription",
            Self::SchemaChanged(_) => "SchemaChanged",
            Self::Offline(_) => "Offline",
        }
    }
}

impl Serialize for ProviderError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("ProviderError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        // `without_url` keeps query strings out of messages; the source chain carries the real cause.
        let e = e.without_url();
        let mut msg = e.to_string();
        let mut src = std::error::Error::source(&e);
        while let Some(s) = src {
            msg.push_str(": ");
            msg.push_str(&s.to_string());
            src = s.source();
        }
        Self::Offline(msg)
    }
}

/// `Retry-After` in seconds; HTTP-date values are ignored (providers here send seconds).
fn retry_after(resp: &reqwest::Response) -> Option<std::time::Duration> {
    let secs: u64 = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(std::time::Duration::from_secs(secs.min(24 * 3600)))
}

/// Maps a response status to a typed error; `Ok(())` for 2xx.
pub fn check_response(resp: &reqwest::Response) -> Result<(), ProviderError> {
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err(ProviderError::TokenExpired),
        429 => Err(ProviderError::RateLimited(retry_after(resp))),
        // A busy server that says when to come back is a rate limit for our purposes.
        503 if retry_after(resp).is_some() => Err(ProviderError::RateLimited(retry_after(resp))),
        code @ 500..=599 => Err(ProviderError::Offline(format!("server error HTTP {code}"))),
        code => Err(ProviderError::SchemaChanged(format!(
            "unexpected HTTP {code}"
        ))),
    }
}
