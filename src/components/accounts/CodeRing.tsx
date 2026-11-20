export function CodeRing({ progress, remaining }: { progress: number; remaining: number }) {
  const radius = 18;
  const circumference = 2 * Math.PI * radius;
  const boundedProgress = Number.isFinite(progress) ? Math.max(0, Math.min(1, progress)) : 0;
  return <span className="code-ring" role="img" aria-label={`${remaining} s`}>
    <svg viewBox="0 0 48 48" aria-hidden="true" className="code-ring-svg">
      <circle cx="24" cy="24" r={radius} fill="none" stroke="var(--line)" strokeWidth="2" />
      <circle cx="24" cy="24" r={radius} fill="none" stroke="var(--accent)" strokeWidth="2" strokeDasharray={circumference} strokeDashoffset={circumference * (1 - boundedProgress)} strokeLinecap="round" />
    </svg>
    <span className="code-ring-value">{remaining}</span>
  </span>;
}
