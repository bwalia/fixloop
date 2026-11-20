import { LogoMark } from './Logo';

const REPO = 'https://github.com/bwalia/fixloop';
const DOCS = `${REPO}/blob/main/docs/getting-started.md`;

// Shared by every page (rendered from the root layout).
export default function Footer() {
  return (
    <footer className="site-footer">
      <div className="container footer-inner">
        <div className="footer-brand">
          <LogoMark size={22} /> <span>Fixloop</span>
        </div>
        <nav aria-label="Footer" className="footer-nav">
          <a href={REPO}>GitHub</a>
          <a href={DOCS}>Getting started</a>
          <a href={`${REPO}/blob/main/docs/config-reference.md`}>Configuration</a>
          <a href={`${REPO}/blob/main/docs/security/threat-model.md`}>Security</a>
          <a href={`${REPO}/blob/main/CONTRIBUTING.md`}>Contribute</a>
          <a href="mailto:support@workstation.co.uk">support@workstation.co.uk</a>
        </nav>
        <p className="footer-sponsor">
          Sponsored by{' '}
          <a href="https://www.workstation.co.uk/en" rel="sponsored noopener">Workstation AI</a>
        </p>
        <p className="footer-legal">© The Fixloop Authors. Apache License 2.0.</p>
      </div>
    </footer>
  );
}
