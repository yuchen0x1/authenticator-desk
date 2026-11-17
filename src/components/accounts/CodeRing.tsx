export function CodeRing({ progress, remaining }: { progress: number; remaining: number }) {
  const radius = 18;
  const circumference = 2 * Math.PI * radius;
  return <span className="relative grid size-11 shrink-0 place-items-center" aria-label={`${remaining} s`}><svg viewBox="0 0 48 48" aria-hidden="true" className="absolute inset-0 -rotate-90"><circle cx="24" cy="24" r={radius} fill="none" stroke="var(--line)" strokeWidth="2"/><circle cx="24" cy="24" r={radius} fill="none" stroke="var(--accent)" strokeWidth="2" strokeDasharray={circumference} strokeDashoffset={circumference * (1 - progress)} strokeLinecap="round"/></svg><span className="relative text-xs tabular-nums">{remaining}</span></span>;
}
