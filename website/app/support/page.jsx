export const metadata = {
  title: 'Support · Fixloop',
  description: 'Get help with the Fixloop app for iPhone and iPad: signing in, missing reviews, notifications and your data.',
  alternates: { canonical: '/support' },
};

const SUPPORT_EMAIL = 'support@workstation.co.uk';
const REPO = 'https://github.com/bwalia/fixloop';
const GETTING_STARTED = `${REPO}/blob/main/docs/getting-started.md`;
const ISSUES = `${REPO}/issues`;
const GITHUB_APPS = 'https://github.com/settings/applications';
const GITHUB_TOKENS = 'https://github.com/settings/tokens';

const Email = () => <a href={`mailto:${SUPPORT_EMAIL}`}>{SUPPORT_EMAIL}</a>;

const FAQ = [
  {
    q: 'How do I sign in?',
    a: (
      <p>
        Choose <strong>Sign in with GitHub</strong>. The app shows a one-time code; open the link, enter the code on
        GitHub&rsquo;s website and approve access. You can also paste a GitHub personal access token instead. Read-only
        access to pull requests is all the app needs.
      </p>
    ),
  },
  {
    q: 'The app shows no reviews. Why?',
    a: (
      <p>
        The app shows reviews that the Fixloop reviewer has posted on pull requests you are involved in. If nobody has
        set up the reviewer on your repositories yet, there is nothing to show. See{' '}
        <a href={GETTING_STARTED}>Getting started</a> to set it up, or choose <strong>Try with sample reviews</strong> on the sign-in screen to see what the
        app looks like with reviews.
      </p>
    ),
  },
  {
    q: 'Why am I not getting notifications?',
    a: (
      <p>
        Turn notifications on in the app and allow them when iOS asks. The app checks GitHub in the background, and iOS
        decides how often that happens, so notifications can arrive later than the review. Low Power Mode and turning
        off Background App Refresh stop the checks.
      </p>
    ),
  },
  {
    q: 'How do I sign out or remove access?',
    a: (
      <p>
        Sign out in the app: this deletes your token and saved settings from the device. To revoke access on GitHub
        as well, go to <a href={GITHUB_APPS}>Applications</a> (if you signed in with GitHub) or{' '}
        <a href={GITHUB_TOKENS}>Personal access tokens</a> (if you pasted a token) in your GitHub settings.
      </p>
    ),
  },
  {
    q: 'Do I need to delete an account?',
    a: (
      <p>
        No. There is no Fixloop account, and the app does not send any data to us. Signing out and deleting the app
        removes everything it stored. See the <a href="/privacy/">privacy policy</a>.
      </p>
    ),
  },
];

export default function Support() {
  return (
    <main className="container prose-page" style={{ padding: '72px 0 96px', maxWidth: 760 }}>
      <h1>Support</h1>
      <p>
        Need help with the Fixloop app for iPhone and iPad? Check the questions below, or email us at <Email />. We
        aim to reply within a few working days.
      </p>

      <h2>Common questions</h2>
      {FAQ.map((item) => (
        <section key={item.q}>
          <h3>{item.q}</h3>
          {item.a}
        </section>
      ))}

      <h2 id="contact">Contact us</h2>
      <p>
        Email <Email />. Please don&rsquo;t send your GitHub token or password. For bugs and feature requests in the
        Fixloop reviewer itself, open an issue on <a href={ISSUES}>GitHub</a>.
      </p>
      <p>
        See also our <a href="/privacy/">privacy policy</a>.
      </p>
    </main>
  );
}
