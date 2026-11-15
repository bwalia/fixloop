export default function NotFound() {
  return (
    <main className="container" style={{ padding: '120px 0', textAlign: 'center' }}>
      <p className="eyebrow">404</p>
      <h1 style={{ fontSize: 'var(--fx-text-3xl)' }}>This page is not in the loop.</h1>
      <p><a className="link-arrow" href="/">Back to Fixloop</a></p>
    </main>
  );
}
