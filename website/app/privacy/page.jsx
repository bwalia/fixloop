export const metadata = {
  title: 'Privacy policy · Fixloop',
  description: 'How the Fixloop iOS app and the Fixloop website handle your data.',
  alternates: { canonical: '/privacy' },
};

const UPDATED = '20 November 2026';

const GITHUB_PRIVACY = 'https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement';
const THREAT_MODEL = 'https://github.com/bwalia/fixloop/blob/main/docs/security/threat-model.md';

export default function Privacy() {
  return (
    <main className="container prose-page" style={{ padding: '72px 0 96px', maxWidth: 760 }}>
      <p className="eyebrow">Last updated {UPDATED}</p>
      <h1>Privacy policy</h1>
      <p>
        This policy covers the Fixloop app for iPhone and iPad (&ldquo;the app&rdquo;) and this website, fixloop.run
        (&ldquo;the website&rdquo;).
      </p>

      <h2>Who we are</h2>
      <p>
        Fixloop is open source software sponsored by Workstation AI. Workstation AI is a trading name of Workstation
        Solutions Ltd, a company registered in England and Wales (company number 11641870). Our registered office is
        The Courtyard, 77-79 Marlowes, Hemel Hempstead, England, HP1 1LF. In this policy, &ldquo;we&rdquo; and
        &ldquo;us&rdquo; mean Workstation Solutions Ltd.
      </p>
      <p>
        For privacy questions, email <a href="mailto:privacy@workstation.co.uk">privacy@workstation.co.uk</a>. For help
        with Fixloop, email <a href="mailto:support@workstation.co.uk">support@workstation.co.uk</a>.
      </p>

      <h2>The app</h2>
      <p>
        The app shows the reviews that Fixloop has posted on GitHub pull requests you are involved in. It has no Fixloop
        account, no analytics, no advertising and no tracking, and it does not send any data to us. We do not run a
        server that the app talks to.
      </p>

      <h3>What the app reads from GitHub</h3>
      <p>
        The app connects directly from your device to GitHub (github.com and api.github.com) and makes only read
        requests. It reads:
      </p>
      <ul>
        <li>your GitHub username, to show who is signed in;</li>
        <li>the pull requests you are involved in, found with GitHub&rsquo;s search;</li>
        <li>the reviews and review comments on those pull requests, so it can show the ones Fixloop posted.</li>
      </ul>
      <p>
        GitHub processes these requests under its own{' '}
        <a href={GITHUB_PRIVACY}>privacy statement</a>.
      </p>

      <h3>Signing in</h3>
      <p>You can sign in in one of two ways. Either way, the result is a GitHub access token that the app uses for the requests above.</p>
      <ul>
        <li>
          <strong>Sign in with GitHub:</strong> the app shows a one-time code, and you approve access on GitHub&rsquo;s
          website. GitHub then issues the app a token. What that token can access is limited by the permissions of the
          Fixloop GitHub App and by your own access on GitHub.
        </li>
        <li>
          <strong>Personal access token:</strong> you create a token on GitHub and paste it into the app. You choose its
          permissions; read-only access to pull requests is all the app needs.
        </li>
      </ul>

      <h3>What is stored on your device</h3>
      <ul>
        <li>
          <strong>Your access token</strong>, in the iOS keychain. It is available only on that device and is not
          included in backups restored to a different device. It is sent only to GitHub.
        </li>
        <li>
          <strong>Your GitHub username</strong>, a list of the reviews you have already been told about, and whether you
          turned notifications on.
        </li>
      </ul>
      <p>
        Pull request titles, review text and code suggestions are held in memory while the app is open and are not
        saved to the device. Signing out deletes your token and the items above. You can also revoke the token at any
        time in your GitHub settings.
      </p>

      <h3>Notifications</h3>
      <p>
        If you turn notifications on, iOS lets the app check GitHub in the background from time to time and show a
        notification on your device when a new Fixloop review appears. The notification is created by the app on your
        device. No push notification service or server is involved.
      </p>

      <h3>Sample mode</h3>
      <p>Sample mode shows reviews built into the app and makes no network requests.</p>

      <h3>Crash reports from Apple</h3>
      <p>
        If you have chosen in your iPhone or iPad settings to share analytics with app developers, Apple may share
        crash reports and usage statistics for the app with its developer. Apple does not identify you in them. You can change this at any
        time in Settings &gt; Privacy &amp; Security &gt; Analytics &amp; Improvements.
      </p>

      <h2>The website</h2>
      <p>
        fixloop.run is a static website. It sets no cookies, uses no analytics and loads nothing from third-party
        services: its fonts are served from fixloop.run. If you choose a light or dark theme, your browser remembers the
        choice in its own local storage; it is not sent to us.
      </p>

      <h3>Server logs</h3>
      <p>
        Requests to the website pass through edge servers in London, which fetch the pages from GitHub Pages. These
        servers record each request in a log: your IP address, the date and time, the page requested, the response
        status and size, the referring page and your browser&rsquo;s user agent. The logs are used only to keep the
        website secure and working and to investigate problems. They are kept only as long as needed for those purposes
        and are deleted automatically on a rolling basis.
      </p>
      <p>
        GitHub, which hosts the pages, also logs visitors&rsquo; IP addresses for security purposes, under its{' '}
        <a href={GITHUB_PRIVACY}>privacy statement</a>.
      </p>
      <p>
        Our legal basis for server logs is our legitimate interest in running a secure and reliable website (Article
        6(1)(f) UK GDPR).
      </p>

      <h2>Self-hosted Fixloop</h2>
      <p>
        If you run the Fixloop reviewer on your own infrastructure, you control the code it reads and the model provider
        it sends that code to. We do not receive any of it. See the{' '}
        <a href={THREAT_MODEL}>security documentation</a>.
      </p>

      <h2>Your rights</h2>
      <p>
        Under UK data protection law you have the right to ask for a copy of your personal data and to ask us to
        correct, delete or restrict it, or to object to how we use it. Email{' '}
        <a href="mailto:privacy@workstation.co.uk">privacy@workstation.co.uk</a>. Because the app does not send us any
        data, the only personal data we may hold about you from Fixloop is in the website&rsquo;s server logs.
      </p>
      <p>
        If you are unhappy with how we have handled your data, you can complain to the Information Commissioner&rsquo;s
        Office at <a href="https://ico.org.uk/make-a-complaint/">ico.org.uk</a>.
      </p>

      <h2>Children</h2>
      <p>The app and website are developer tools and are not directed at children.</p>

      <h2>Changes</h2>
      <p>If this policy changes, we will update this page and the date at the top.</p>
    </main>
  );
}
