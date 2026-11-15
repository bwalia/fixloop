// The loop mark: a ring broken by an arrowhead, with one accent cell where the
// loop closes.
export function LogoMark({ size = 28 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true" className="logo-mark">
      <path d="M26 16a10 10 0 1 1-4.2-8.15" fill="none" stroke="var(--fx-logo-ink)" strokeWidth="3" strokeLinecap="round" />
      <path d="M17.5 4.5l5 3.4-3.6 4.9" fill="none" stroke="var(--fx-logo-ink)" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="22.5" y="14" width="5" height="5" rx="1" fill="var(--fx-logo-accent)" />
    </svg>
  );
}

export function Logo() {
  return (
    <span className="logo">
      <LogoMark />
      <span className="logo-word">Fixloop</span>
    </span>
  );
}
