/**
 * The Tideline mark: a flow (the wave) crossing the tide line (the horizon), with a pin
 * on the crest for the attribution that names who moved it. Pure SVG in `currentColor`
 * plus the accent, so it is crisp at any size and follows the colour scheme.
 */
export function LogoMark({ className = "size-7" }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden="true" fill="none">
      <rect x="0.75" y="0.75" width="30.5" height="30.5" rx="9.25" fill="currentColor" fillOpacity="0.06" />
      <rect x="0.75" y="0.75" width="30.5" height="30.5" rx="9.25" stroke="currentColor" strokeOpacity="0.16" strokeWidth="1.5" />
      <path d="M7 23.25h18" stroke="currentColor" strokeOpacity="0.4" strokeWidth="1.6" strokeLinecap="round" />
      <path
        d="M6.5 18c2.6 0 3.4-4.5 6.1-4.5S16 18 18.7 18s3.6-4.5 6.8-4.5"
        className="stroke-accent"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="21.9" cy="8.6" r="2.15" className="fill-accent" />
    </svg>
  );
}

export function Logo() {
  return (
    <span className="flex items-center gap-2.5">
      <LogoMark />
      <span className="text-[15px] font-semibold tracking-[-0.02em]">Tideline</span>
    </span>
  );
}
