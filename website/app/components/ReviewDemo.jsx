// What a Fixloop review looks like on a pull request. Based on a real run
// against a seeded bug, with a local 30B model.
export default function ReviewDemo() {
  return (
    <div className="demo" aria-label="Example review comment on a pull request">
      <div className="demo-bar">
        <span className="demo-dots" aria-hidden="true"><i /><i /><i /></span>
        <span className="demo-path">src/util.rs</span>
        <span className="demo-pr">PR #1</span>
      </div>
      <pre className="demo-code" aria-label="Diff">
        <span className="ln">5</span><span className="ctx">  pub fn last(items: &amp;[i32]) -&gt; Option&lt;i32&gt; {'{'}</span>{'\n'}
        <span className="ln">6</span><span className="ctx">      if items.is_empty() {'{'} return None; {'}'}</span>{'\n'}
        <span className="ln">7</span><span className="add">+     Some(items[items.len()])</span>{'\n'}
        <span className="ln">8</span><span className="ctx">  {'}'}</span>
      </pre>
      <div className="demo-comment">
        <div className="demo-who">
          <span className="demo-avatar" aria-hidden="true">fx</span>
          <strong>fixloop</strong>
          <span className="chip chip-danger">high</span>
          <span className="chip">bug</span>
          <span className="demo-conf">confidence 0.95</span>
        </div>
        <p className="demo-title">Off-by-one: index out of bounds</p>
        <p>
          <code>items.len()</code> is one past the last index, so this panics on every non-empty slice.
        </p>
        <div className="demo-suggest">
          <div className="demo-suggest-head">Suggested change</div>
          <pre>
            <span className="del">-     Some(items[items.len()])</span>
            <span className="add">+     Some(items[items.len() - 1])</span>
          </pre>
          <div className="demo-suggest-foot"><span className="fake-btn">Commit suggestion</span></div>
        </div>
      </div>
      <div className="demo-check">
        <span className="check-icon" aria-hidden="true">✕</span>
        <span><strong>Fixloop review</strong>: 2 findings: 1 critical (SQL injection in <code>app/users.py</code>), 1 high</span>
      </div>
    </div>
  );
}
