import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

export function Row({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div className="srow">
      <div className="srow-text">
        <div className="srow-title">{title}</div>
        {hint && <div className="srow-hint">{hint}</div>}
      </div>
      <div className="srow-control">{children}</div>
    </div>
  );
}


export function Segmented<T extends string | number>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState<{ left: number; width: number } | null>(null);
  const [ready, setReady] = useState(false);
  const labels = options.map((o) => o.label).join("|");

  // The thumb slides to the selected segment; it is measured, so any label width works.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const on = el.querySelector<HTMLElement>('[aria-checked="true"]');
      if (on) setThumb({ left: on.offsetLeft, width: on.offsetWidth });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [value, labels]);

  // No slide on the very first placement, only on later changes.
  useEffect(() => {
    if (thumb && !ready) {
      const id = requestAnimationFrame(() => setReady(true));
      return () => cancelAnimationFrame(id);
    }
  }, [thumb, ready]);

  return (
    <div className="segmented" role="radiogroup" aria-label={label} ref={ref}>
      {thumb && (
        <span
          className={ready ? "seg-thumb seg-thumb-anim" : "seg-thumb"}
          style={{ transform: `translateX(${thumb.left}px)`, width: thumb.width }}
          aria-hidden
        />
      )}
      {options.map((o) => (
        <button
          key={String(o.value)}
          role="radio"
          aria-checked={o.value === value}
          className={o.value === value ? "seg seg-on" : "seg"}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
