export const metadata = {
  title: 'Privacy policy · Fixloop',
  description: 'How the Fixloop iOS app and the Fixloop website handle your data.',
  alternates: { canonical: '/privacy' },
};

const UPDATED = '20 November 2026';

export default function Privacy() {
  return (
    <main className="container prose-page" style={{ padding: '72px 0 96px', maxWidth: 760 }}>
      <p className="eyebrow">Last updated {UPDATED}</p>
      <h1>Privacy policy</h1>
      <p>
        This policy covers the Fixloop app for iPhone and iPad (&ldquo;the app&rdquo;) and this website, fixloop.run.
        Fixloop is open source software sponsored by Workstation AI. If you have a question about this policy, email{' '}
        <a href="mailto:support@workstation.co.uk">support@workstation.co.uk</a>.
      </p>

      <h2>The app</h2>
      <p>The app shows the reviews that Fixloop has posted on GitHub pull requests you are involved in.</p>
      <ul>
        <li>
          <strong>We don&rsquo;t collect your data.</strong> The app has no Fixloop account, no analytics, no advertising
          and no tracking. We don&rsquo;t run a server that the app talks to.
        </li>
        <li>
          <strong>It talks only to GitHub.</strong> When you sign in, the app gets a GitHub access token from GitHub and
          uses it to read your pull requests and their review comments directly from GitHub&rsquo;s API. GitHub&rsquo;s{' '}
          <a href="https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement">privacy statement</a>{' '}
          applies to that data.
        </li>
        <li>
          <strong>Your token stays on your device.</strong> It is kept in the iOS keychain on that device and is never
          sent anywhere except GitHub. Signing out deletes it. You can also revoke it at any time in your GitHub settings.
        </li>
        <li>
          <strong>Notifications are created on your device.</strong> If you turn them on, iOS lets the app check GitHub
          in the background and show a local notification. No push service or server is involved.
        </li>
        <li>
          <strong>Sample mode</strong> uses reviews built into the app and makes no network requests.
        </li>
      </ul>

      <h2>This website</h2>
      <p>
        fixloop.run is a static site. It sets no cookies and uses no analytics. Your browser stores your light or dark
        theme choice locally. The hosting provider keeps standard server logs, such as IP address and pages requested,
        for security and operations.
      </p>

      <h2>Self-hosted Fixloop</h2>
      <p>
        If you run the Fixloop reviewer on your own infrastructure, you control the code it reads and the model
        provider it sends that code to. See the{' '}
        <a href="https://github.com/bwalia/fixloop/blob/main/docs/security/threat-model.md">security documentation</a>.
      </p>

      <h2>Children</h2>
      <p>The app is a developer tool and is not directed at children.</p>

      <h2>Changes</h2>
      <p>If this policy changes, we will update this page and the date at the top.</p>
    </main>
  );
}
