// The six stages on a ring. A pulse travels the loop; stages that ship today
// are drawn solid, planned ones dashed.
const STAGES = [
  { name: 'Review', live: true },
  { name: 'Find', live: true },
  { name: 'File', live: false },
  { name: 'Fix', live: false },
  { name: 'Check', live: false },
  { name: 'Close', live: false },
];

const R = 150;
const C = 200;

export default function LoopDiagram() {
  return (
    <figure className="loop" aria-label="The Fixloop cycle: Review, Find, File, Fix, Check, Close">
      <svg viewBox="0 0 400 400" role="img" aria-hidden="true">
        <circle cx={C} cy={C} r={R} className="loop-track" />
        <circle cx={C} cy={C} r={R} className="loop-progress" pathLength="100" />
        {STAGES.map((s, i) => {
          const a = (i / STAGES.length) * Math.PI * 2 - Math.PI / 2;
          const x = C + R * Math.cos(a);
          const y = C + R * Math.sin(a);
          return (
            <g key={s.name} className={`loop-node ${s.live ? 'is-live' : 'is-planned'}`} style={{ '--i': i }}>
              <circle cx={x} cy={y} r="34" />
              <text x={x} y={y + 5} textAnchor="middle">{s.name}</text>
            </g>
          );
        })}
        <text x={C} y={C - 6} textAnchor="middle" className="loop-center-title">fixloop</text>
        <text x={C} y={C + 18} textAnchor="middle" className="loop-center-sub">one agent, one loop</text>
      </svg>
      <figcaption className="loop-legend">
        <span><i className="dot dot-live" /> Available now</span>
        <span><i className="dot dot-planned" /> On the roadmap</span>
      </figcaption>
    </figure>
  );
}
