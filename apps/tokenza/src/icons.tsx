import type { SVGProps } from "react";

const base = (size: number): SVGProps<SVGSVGElement> => ({
  width: size,
  height: size,
  viewBox: "0 0 20 20",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
});

export const RefreshIcon = ({ size = 16 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d="M16 10a6 6 0 1 1-2-4.5M16 3v3.5h-3.5" />
  </svg>
);

/** Outline of an 8-tooth cog: tooth tips at radius 8.2, valleys at 6.3, centered on (10, 10). */
const GEAR_PATH = (() => {
  const pts: string[] = [];
  const at = (deg: number, r: number) => {
    const a = ((deg - 90) * Math.PI) / 180;
    return `${(10 + r * Math.cos(a)).toFixed(2)} ${(10 + r * Math.sin(a)).toFixed(2)}`;
  };
  for (let i = 0; i < 8; i++) {
    const c = i * 45;
    pts.push(at(c - 15, 6.3), at(c - 8, 8.2), at(c + 8, 8.2), at(c + 15, 6.3));
  }
  return `M${pts.join("L")}Z`;
})();

export const GearIcon = ({ size = 16 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d={GEAR_PATH} />
    <circle cx="10" cy="10" r="2.4" />
  </svg>
);

/** Six-dot drag handle. */
export const GripIcon = ({ size = 16 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor" aria-hidden>
    <circle cx="5.5" cy="3.5" r="1.2" />
    <circle cx="10.5" cy="3.5" r="1.2" />
    <circle cx="5.5" cy="8" r="1.2" />
    <circle cx="10.5" cy="8" r="1.2" />
    <circle cx="5.5" cy="12.5" r="1.2" />
    <circle cx="10.5" cy="12.5" r="1.2" />
  </svg>
);

export const LockIcon = ({ size = 20 }: { size?: number }) => (
  <svg {...base(size)}>
    <rect x="4" y="9" width="12" height="8" rx="2" />
    <path d="M7 9V6a3 3 0 0 1 6 0v3" />
  </svg>
);

export const AlertIcon = ({ size = 20 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d="M10 3 18 17H2L10 3Z" />
    <path d="M10 8v4" />
    <circle cx="10" cy="14.3" r=".6" fill="currentColor" />
  </svg>
);

export const OfflineIcon = ({ size = 20 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d="M3 8.5a10 10 0 0 1 14 0M6 11.7a6 6 0 0 1 8 0" />
    <circle cx="10" cy="15" r=".7" fill="currentColor" />
    <path d="M3 3l14 14" />
  </svg>
);

export const SearchIcon = ({ size = 20 }: { size?: number }) => (
  <svg {...base(size)}>
    <circle cx="9" cy="9" r="5" />
    <path d="m13 13 4 4" />
  </svg>
);

export const ChevronUp = ({ size = 14 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d="m5 12 5-5 5 5" />
  </svg>
);

export const ChevronDown = ({ size = 14 }: { size?: number }) => (
  <svg {...base(size)}>
    <path d="m5 8 5 5 5-5" />
  </svg>
);
