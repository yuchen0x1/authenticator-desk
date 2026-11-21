import { useEffect, useState } from "react";
import type { AccountView, CodeView } from "../../lib/types";
import { futureT0MessageForCopyLabel, isFutureTotp } from "../../i18n/futureT0";
import { CodeRing } from "./CodeRing";

type Props = {
  account: AccountView;
  code?: CodeView;
  selected: boolean;
  masked: boolean;
  copyAvailable?: boolean;
  advancing?: boolean;
  favoriting?: boolean;
  t: { copy: string; edit: string; favorite: string; unfavorite: string; select: string; hotp: string; advanceHotp: string; hiddenCode: string; futureT0?: string };
  onSelect: (id: string, checked: boolean) => void;
  onEdit: (account: AccountView) => void;
  onFavorite: (account: AccountView) => void;
  onCopy: (account: AccountView) => void;
  onAdvance: (account: AccountView) => void;
};

export function AccountCard({ account, code, selected, masked, copyAvailable = false, advancing = false, favoriting = false, t, onSelect, onEdit, onFavorite, onCopy, onAdvance }: Props) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (masked || account.kind !== "totp") return;
    let timer: number;
    const tick = () => {
      setNow(Date.now());
      timer = window.setTimeout(tick, 1000 - Date.now() % 1000);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, [account.kind, masked]);
  const currentUnix = Math.floor(Math.max(now, Date.now()) / 1000);
  const futureT0 = isFutureTotp(account, currentUnix - (code?.clock_offset_secs ?? 0));
  const remaining = code ? Math.max(0, code.remaining_secs - Math.max(0, currentUnix - code.clock_offset_secs - code.generated_at)) : 0;
  const visibleCode = !masked && !futureT0 && !advancing && code && (account.kind === "hotp" || remaining > 0) ? code : undefined;
  // Every repeated action carries both service and account identity.
  const name = `${account.issuer} · ${account.account_name}`;
  function copy() {
    if (copyAvailable && visibleCode && (account.kind === "hotp" || Math.floor(Date.now() / 1000) - visibleCode.clock_offset_secs < visibleCode.generated_at + visibleCode.remaining_secs)) onCopy(account);
  }
  return <article className="account-row" role="listitem" aria-label={name}>
    <input aria-label={`${t.select}: ${name}`} type="checkbox" checked={selected} onChange={(event) => onSelect(account.id, event.target.checked)} />
    <div className="account-identity">
      <div className="account-service">
        <span className="service-mark" aria-hidden="true">{account.issuer.slice(0, 1).toUpperCase()}</span>
        <strong className="account-issuer" title={account.issuer}>{account.issuer}</strong>
        {account.kind === "hotp" && <span className="type-label">{t.hotp}</span>}
      </div>
      <p className="account-name" title={account.account_name}>{account.account_name}</p>
      <div className="account-tags">{account.tags.map((tag) => <span className="tag" key={tag}>{tag}</span>)}</div>
    </div>
    <div className="account-code-actions">
      {futureT0 && !masked
        ? <span className="future-t0-status text-sm text-[var(--muted)] workspace-wrap">{t.futureT0 ?? futureT0MessageForCopyLabel(t.copy)}</span>
        : <code className="otp-code">{masked ? "•".repeat(account.digits) : visibleCode?.code.replace(/(.{3})/, "$1 ") ?? "-".repeat(account.digits)}</code>}
      {masked && <span className="visually-hidden">{t.hiddenCode}</span>}
      {account.kind === "totp"
        ? !futureT0 && <CodeRing progress={visibleCode ? remaining / account.period_secs : 0} remaining={visibleCode ? remaining : 0} />
        : <button className="icon-button" title={`${t.advanceHotp}: ${name}`} aria-label={`${t.advanceHotp}: ${name}`} disabled={masked || advancing} aria-busy={advancing} onClick={() => { if (!masked && !advancing) onAdvance(account); }}>＋</button>}
      <button className="icon-button" aria-label={`${account.favorite ? t.unfavorite : t.favorite}: ${name}`} title={account.favorite ? t.unfavorite : t.favorite} disabled={favoriting} aria-busy={favoriting} onClick={() => onFavorite(account)}>{account.favorite ? "★" : "☆"}</button>
      <button className="icon-button" aria-label={`${t.edit}: ${name}`} title={t.edit} disabled={advancing} onClick={() => { if (!advancing) onEdit(account); }}>✎</button>
      <button className="icon-button" aria-label={`${t.copy}: ${name}`} title={t.copy} disabled={!copyAvailable || !visibleCode} onClick={copy}>⧉</button>
    </div>
  </article>;
}
