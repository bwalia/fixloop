import { Logo, LogoMark } from './components/Logo';
import ThemeToggle from './components/ThemeToggle';
import CopyCommand from './components/CopyCommand';
import LoopDiagram from './components/LoopDiagram';
import ReviewDemo from './components/ReviewDemo';

const REPO = 'https://github.com/bwalia/fixloop';
const DOCS = `${REPO}/blob/main/docs/getting-started.md`;
const INSTALL = 'cargo install --locked --git https://github.com/bwalia/fixloop fixloop-cli';

const DEV_POINTS = [
  ['Comments on the lines you changed', 'Each finding sits on the changed line, with a severity, a confidence score and a suggested fix you can commit in one click.'],
  ['Review before you push', 'Run fixloop review in your repository. It checks your branch against main and prints findings. Nothing is posted.'],
  ['Your rules, in plain words', 'Write team conventions as short Markdown files in .fixloop/rules. Every review checks against them.'],
  ['Quiet by default', 'Findings below your confidence or severity threshold are dropped. No style nits unless you ask for them.'],
];

const LEAD_POINTS = [
  ['A clear signal on every PR', 'A pass, warn or fail check on every pull request, so problems are caught before a human reviewer spends time on them.'],
  ['Spend you can cap', 'Token and dollar budgets per job and per day. Cost is recorded for every review.'],
  ['Your code stays where you decide', 'Self-host it. Mark a repository local-only and its code is only ever sent to models you run.'],
  ['No lock-in', 'Apache-2.0, vendor-neutral, any model provider. Built to become a CNCF project.'],
];

const MODELS = ['Claude', 'OpenAI', 'Gemini', 'OpenRouter', 'Azure OpenAI', 'Ollama', 'vLLM', 'llama.cpp', 'LM Studio', 'NVIDIA NIM'];

const SAFETY = [
  ['Secrets never reach a model', 'Keys and tokens in a diff are redacted before any model call, and a committed secret is reported as a critical finding.'],
  ['A PR cannot weaken its own review', 'Settings and rules are read from the base branch, never from the pull request.'],
  ['Untrusted input stays data', 'Code, comments and PR text are passed to the model as data, never as instructions. Output is sanitised before it is posted.'],
  ['Least privilege', 'The GitHub App you create reads code and writes reviews and checks. Nothing else. Only people with write access can run its commands.'],
];

const STEPS = [
  ['Install', INSTALL],
  ['Pick a model', 'fixloop --config fixloop.yaml llm test'],
  ['Review your branch', 'fixloop --config fixloop.yaml review'],
  ['Connect GitHub', 'fixloop github create-app --name my-fixloop'],
];

const ROADMAP = [
  ['Done', 'Model layer', 'Any provider, routing, fallback, budgets'],
  ['Done', 'Reviewer', 'GitHub App, inline reviews, check runs'],
  ['Next', 'Issue filing', 'GitHub Issues and Jira, with dedupe'],
  ['Planned', 'Fixer', 'Sandboxed, test-gated fix pull requests'],
  ['Planned', 'Dashboard', 'Findings, costs and audit in one place'],
  ['Planned', 'Cloud native', 'Helm, operator, signed images, SBOM'],
];

function Header() {
  return (
    <header className="site-header">
      <div className="container header-inner">
        <a href="#top" className="brand" aria-label="Fixloop home"><Logo /></a>
        <nav aria-label="Main" className="nav">
          <a href="#how">How it works</a>
          <a href="#developers">Developers</a>
          <a href="#teams">Teams</a>
          <a href="#models">Models</a>
          <a href="#security">Security</a>
          <a href={DOCS}>Docs</a>
        </nav>
        <div className="header-actions">
          <ThemeToggle />
          <a className="btn btn-small btn-ghost" href={REPO}>GitHub</a>
        </div>
      </div>
    </header>
  );
}

function Hero() {
  return (
    <section className="hero" id="top">
      <div className="container hero-grid">
        <div className="hero-copy">
          <p className="eyebrow">Open source · Apache-2.0 · Written in Rust</p>
          <h1>AI code review that closes the bug loop.</h1>
          <p className="lead">
            Fixloop reviews every pull request, comments on the lines that changed and suggests the fix. It runs on your
            infrastructure, with the model you choose. Issue filing and test-gated fix pull requests come next.
          </p>
          <div className="hero-ctas">
            <a className="btn btn-primary" href={DOCS}>Get started</a>
            <a className="btn btn-ghost" href={REPO}>Star on GitHub</a>
          </div>
          <CopyCommand command={INSTALL} />
        </div>
        <div className="hero-art">
          <ReviewDemo />
        </div>
      </div>
    </section>
  );
}

function How() {
  return (
    <section className="section" id="how">
      <div className="container split">
        <div>
          <p className="eyebrow">How it works</p>
          <h2>Review → Find → File → Fix → Check → Close</h2>
          <p className="lead-sm">
            One agent carries a bug from the pull request where it appears to the change that closes it. Today it reviews
            and finds. Each next stage ships as its own release.
          </p>
          <ol className="stage-list">
            <li><strong>Review.</strong> On every opened or updated pull request, or on <code>/fixloop review</code>. <span className="tag tag-live">Available</span></li>
            <li><strong>Find.</strong> Bugs, security issues and broken team rules, on the changed lines only. <span className="tag tag-live">Available</span></li>
            <li><strong>File.</strong> Open an issue in GitHub or Jira, without duplicates. <span className="tag">Next</span></li>
            <li><strong>Fix.</strong> Write the fix in a sandbox. <span className="tag">Planned</span></li>
            <li><strong>Check.</strong> Build, test and lint must pass before a fix PR opens. <span className="tag">Planned</span></li>
            <li><strong>Close.</strong> Link the fix, close the issue, record the cost. <span className="tag">Planned</span></li>
          </ol>
        </div>
        <LoopDiagram />
      </div>
    </section>
  );
}

function Points({ id, eyebrow, title, intro, points }) {
  return (
    <section className="section" id={id}>
      <div className="container">
        <p className="eyebrow">{eyebrow}</p>
        <h2>{title}</h2>
        <p className="lead-sm narrow">{intro}</p>
        <div className="cards">
          {points.map(([h, p]) => (
            <article className="card" key={h}>
              <h3>{h}</h3>
              <p>{p}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function Models() {
  return (
    <section className="section section-sunken" id="models">
      <div className="container split split-even">
        <div>
          <p className="eyebrow">Bring your own model</p>
          <h2>Hosted, local or both.</h2>
          <p className="lead-sm">
            Model names live in one config file. List a fallback, and if the first model is down or rate limited, the next
            one takes over. Run fully offline with Ollama and no code leaves your network.
          </p>
          <ul className="model-chips" aria-label="Supported providers">
            {MODELS.map((m) => <li key={m}>{m}</li>)}
          </ul>
        </div>
        <pre className="code-block" aria-label="Example configuration">
<span className="c"># fixloop.yaml: a hosted model with a local fallback</span>{'\n'}
<span className="k">llm</span>:{'\n'}
{'  '}<span className="k">providers</span>:{'\n'}
{'    '}<span className="k">anthropic</span>: {'{'} kind: anthropic, api_key_env: <span className="s">ANTHROPIC_API_KEY</span> {'}'}{'\n'}
{'    '}<span className="k">ollama</span>:    {'{'} kind: openai_compatible, preset: ollama {'}'}{'\n'}
{'  '}<span className="k">models</span>:{'\n'}
{'    '}<span className="k">claude</span>: {'{'} provider: anthropic, model: <span className="s">"claude-sonnet-5-5"</span> {'}'}{'\n'}
{'    '}<span className="k">local</span>:  {'{'} provider: ollama, model: <span className="s">"qwen3:30b-a3b"</span>  {'}'}{'\n'}
{'  '}<span className="k">routes</span>:{'\n'}
{'    '}<span className="k">review</span>: [claude, local]{'\n'}
{'  '}<span className="k">budgets</span>:{'\n'}
{'    '}<span className="k">daily</span>: {'{'} max_cost_usd: <span className="n">50</span> {'}'}
        </pre>
      </div>
    </section>
  );
}

function Start() {
  return (
    <section className="section" id="start">
      <div className="container">
        <p className="eyebrow">Get started</p>
        <h2>Reviewing your first pull request in about 15 minutes.</h2>
        <ol className="steps">
          {STEPS.map(([h, cmd], i) => (
            <li key={h} className="step">
              <span className="step-n">{i + 1}</span>
              <div>
                <h3>{h}</h3>
                <code className="step-cmd">{cmd}</code>
              </div>
            </li>
          ))}
        </ol>
        <p><a className="link-arrow" href={DOCS}>Read the full guide</a></p>
      </div>
    </section>
  );
}

function Roadmap() {
  return (
    <section className="section section-sunken" id="roadmap">
      <div className="container">
        <p className="eyebrow">Roadmap</p>
        <h2>Built in the open, one stage at a time.</h2>
        <div className="roadmap">
          {ROADMAP.map(([status, h, p]) => (
            <div className={`road road-${status.toLowerCase()}`} key={h}>
              <span className="road-status">{status}</span>
              <h3>{h}</h3>
              <p>{p}</p>
            </div>
          ))}
        </div>
        <p><a className="link-arrow" href={`${REPO}/blob/main/ROADMAP.md`}>Full roadmap</a></p>
      </div>
    </section>
  );
}

function CtaBand() {
  return (
    <section className="cta-band">
      <div className="container cta-inner">
        <div>
          <h2>Put a reviewer on every pull request.</h2>
          <p>Free and open source. Self-hosted. Your model, your rules.</p>
        </div>
        <div className="hero-ctas">
          <a className="btn btn-primary" href={DOCS}>Get started</a>
          <a className="btn btn-night" href={REPO}>View the code</a>
        </div>
      </div>
    </section>
  );
}

function Footer() {
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
        </nav>
        <p className="footer-legal">© The Fixloop Authors. Apache License 2.0.</p>
      </div>
    </footer>
  );
}

export default function Home() {
  return (
    <>
      <a className="skip" href="#how">Skip to content</a>
      <Header />
      <main>
        <Hero />
        <How />
        <Points
          id="developers"
          eyebrow="For developers"
          title="A second reviewer that reads every line you changed."
          intro="Fixloop looks for real defects: off-by-ones, injection, unhandled errors, broken team rules. It tells you where, why and how to fix it."
          points={DEV_POINTS}
        />
        <Points
          id="teams"
          eyebrow="For engineering leads"
          title="Consistent review on every repository, at a cost you set."
          intro="Fixloop gives every pull request the same first pass, whoever wrote it and whenever it lands. You choose the model, the limits and where it runs."
          points={LEAD_POINTS}
        />
        <Models />
        <Points
          id="security"
          eyebrow="Security"
          title="Designed for code you cannot afford to leak."
          intro="An AI reviewer reads untrusted input by definition. Fixloop is built on that assumption."
          points={SAFETY}
        />
        <Start />
        <Roadmap />
        <CtaBand />
      </main>
      <Footer />
    </>
  );
}
