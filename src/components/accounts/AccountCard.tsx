import type { AccountView, CodeView } from "../../lib/types";
import { CodeRing } from "./CodeRing";

type Props = { account: AccountView; code?: CodeView; selected: boolean; masked: boolean; copyAvailable?: boolean; t: { copy: string; edit: string; favorite: string; unfavorite: string; select: string; hotp: string }; onSelect: (id: string, checked: boolean) => void; onEdit: (account: AccountView) => void; onFavorite: (account: AccountView) => void; onCopy: (account: AccountView) => void; onAdvance: (account: AccountView) => void };
export function AccountCard({ account, code, selected, masked, copyAvailable = false, t, onSelect, onEdit, onFavorite, onCopy, onAdvance }: Props) {
  return <article className="account-row">
    <input aria-label={t.select + " " + account.issuer} type="checkbox" checked={selected} onChange={(event) => onSelect(account.id, event.target.checked)} />
    <div className="min-w-0 flex-1"><div className="flex items-center gap-2"><span className="service-mark" aria-hidden="true">{account.issuer.slice(0, 1).toUpperCase()}</span><strong className="truncate">{account.issuer}</strong>{account.kind === "hotp" && <span className="type-label">{t.hotp}</span>}</div><p className="mt-1 truncate text-sm text-[var(--muted)]">{account.account_name}</p><div className="mt-2 flex flex-wrap gap-1">{account.tags.map((tag) => <span className="tag" key={tag}>{tag}</span>)}</div></div>
    <div className="flex items-center gap-3"><code className="otp-code" aria-label={masked ? undefined : code?.code}>{masked ? "•".repeat(account.digits) : code?.code?.replace(/(.{3})/, "$1 " ) ?? "-".repeat(account.digits)}</code>{account.kind === "totp" ? <CodeRing progress={code?.progress ?? 0} remaining={code?.remaining_secs ?? 0}/> : <button className="icon-button" title={t.hotp} aria-label={t.hotp} onClick={() => onAdvance(account)}>＋</button>}
    <button className="icon-button" aria-label={account.favorite ? t.unfavorite : t.favorite} title={account.favorite ? t.unfavorite : t.favorite} onClick={() => onFavorite(account)}>{account.favorite ? "★" : "☆"}</button><button className="icon-button" aria-label={t.edit} title={t.edit} onClick={() => onEdit(account)}>✎</button><button className="icon-button" aria-label={t.copy} title={t.copy} disabled={!copyAvailable || masked || !code} onClick={() => onCopy(account)}>⧉</button></div>
  </article>;
}
