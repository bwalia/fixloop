import { Bricolage_Grotesque, Instrument_Sans, IBM_Plex_Mono } from 'next/font/google';
import './theme.css';
import './globals.css';

const display = Bricolage_Grotesque({ subsets: ['latin'], variable: '--font-fx-display', display: 'swap', axes: ['opsz'] });
const body = Instrument_Sans({ subsets: ['latin'], variable: '--font-fx-body', display: 'swap' });
const mono = IBM_Plex_Mono({ subsets: ['latin'], weight: ['400', '500', '600'], variable: '--font-fx-mono', display: 'swap' });

const SITE = 'https://fixloop.run';
const TITLE = 'Fixloop: AI code review that closes the bug loop';
const DESCRIPTION =
  'Open source AI agent that reviews pull requests on the changed lines, with fixes you can apply. Any model: Claude, OpenAI, Gemini or fully local. Self-hosted, Apache-2.0.';

export const metadata = {
  metadataBase: new URL(SITE),
  title: TITLE,
  description: DESCRIPTION,
  alternates: { canonical: '/' },
  openGraph: { type: 'website', url: SITE, siteName: 'Fixloop', title: TITLE, description: DESCRIPTION },
  twitter: { card: 'summary', title: TITLE, description: DESCRIPTION },
  icons: { icon: '/icon.svg' },
};

export const viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: light)', color: '#f7f6f2' },
    { media: '(prefers-color-scheme: dark)', color: '#0e1113' },
  ],
};

// Runs before first paint: localStorage "fx-theme" = light | dark | system.
const THEME_SCRIPT = `(function(){var d=document.documentElement,K='fx-theme',m=window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)');function g(){try{var v=localStorage.getItem(K);return v==='light'||v==='dark'?v:'system'}catch(e){return'system'}}function a(p){var t=p==='system'?(m&&m.matches?'dark':'light'):p;d.setAttribute('data-theme',t);d.setAttribute('data-theme-mode',p);return t}a(g());if(m&&m.addEventListener)m.addEventListener('change',function(){if(g()==='system')a('system')});window.__fxTheme={get:g,set:function(p){try{localStorage.setItem(K,p)}catch(e){}return a(p)}}})();`;

export default function RootLayout({ children }) {
  return (
    <html lang="en" suppressHydrationWarning className={`${display.variable} ${body.variable} ${mono.variable}`}>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_SCRIPT }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
