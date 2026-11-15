'use client';
import { useEffect, useState } from 'react';

const NEXT = { system: 'light', light: 'dark', dark: 'system' };
const LABEL = { system: 'Theme: system', light: 'Theme: light', dark: 'Theme: dark' };

export default function ThemeToggle() {
  const [mode, setMode] = useState('system');
  useEffect(() => setMode(window.__fxTheme?.get() ?? 'system'), []);
  const next = () => {
    const m = NEXT[mode];
    window.__fxTheme?.set(m);
    setMode(m);
  };
  return (
    <button type="button" className="theme-toggle" onClick={next} aria-label={`${LABEL[mode]}. Switch theme.`} title={LABEL[mode]}>
      {mode === 'dark' ? (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" /></svg>
      ) : mode === 'light' ? (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><circle cx="12" cy="12" r="4" fill="none" stroke="currentColor" strokeWidth="1.8" /><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /></svg>
      ) : (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><rect x="3" y="4.5" width="18" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="1.8" /><path d="M8.5 20h7M12 16.5V20" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /></svg>
      )}
    </button>
  );
}
