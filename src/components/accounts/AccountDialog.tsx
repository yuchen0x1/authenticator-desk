import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import type { AccountInput, AccountView, CategoryView, OtpAlgorithm } from "../../lib/types";
import { accountCommands } from "../../lib/tauri";
import { focusDialog, trapDialogFocus } from "../common/dialogFocus";
import { useModalIsolation } from "../common/useModalIsolation";

type Props = { account?: AccountView; categories: CategoryView[]; defaults: { algorithm: OtpAlgorithm; digits: number; period: number }; t: Record<string, string>; onClose: () => void; onSave: (input: AccountInput) => Promise<void> };
export function AccountDialog({ account, categories, defaults, t, onClose, onSave }: Props) {
  const [issuer, setIssuer] = useState(account?.issuer ?? "");
  const [name, setName] = useState(account?.account_name ?? "");
  const [secret, setSecret] = useState("");
  const secretRef = useRef("");
  const [algorithm, setAlgorithm] = useState<OtpAlgorithm>(account?.algorithm ?? defaults.algorithm);
  const [digits, setDigits] = useState(account?.digits ?? defaults.digits);
  const [kind, setKind] = useState(account?.kind ?? "totp");
  const [period, setPeriod] = useState(account?.period_secs ?? defaults.period);
  const [t0, setT0] = useState(account?.t0 ?? "0");
  const [counter, setCounter] = useState(account?.hotp_counter ?? "0");
  const [category, setCategory] = useState(account?.category_id ?? "");
  const [tags, setTags] = useState(account?.tags.join(", ") ?? "");
  const [notes, setNotes] = useState("");
  const [detailsReady, setDetailsReady] = useState(!account);
  const [detailsLoading, setDetailsLoading] = useState(Boolean(account));
  const [detailsError, setDetailsError] = useState(false);
  const [favorite, setFavorite] = useState(account?.favorite ?? false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(document.activeElement instanceof HTMLElement ? document.activeElement : null);
  const wasReady = useRef(false);
  useModalIsolation(true, dialogRef, returnFocus);
  const retryDetails = useRef<() => void>(() => {});

  useEffect(() => {
    let active = true;
    let loading = false;
    secretRef.current = "";
    setSecret("");
    setDetailsReady(!account);
    setDetailsLoading(Boolean(account));
    setDetailsError(false);
    if (account) {
      const load = async () => {
        if (!active || loading) return;
        loading = true;
        setDetailsLoading(true);
        setDetailsError(false);
        try {
          const details = await accountCommands.details(account.id);
          if (active) {
            setNotes(details.notes);
            setDetailsReady(true);
          }
        } catch {
          if (active) setDetailsError(true);
        } finally {
          loading = false;
          if (active) setDetailsLoading(false);
        }
      };
      retryDetails.current = () => { void load(); };
      void load();
    }
    return () => {
      active = false;
      retryDetails.current = () => {};
      secretRef.current = "";
    };
  }, [account?.id]);

  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const active = document.activeElement;
    if (detailsReady && !saving && !wasReady.current) {
      const first = dialog.querySelector<HTMLElement>("form input:not([disabled]), form select:not([disabled]), form textarea:not([disabled])");
      (first ?? dialog).focus();
    } else if (!dialog.contains(active) || active === dialog || active?.matches(":disabled")) {
      focusDialog(dialog);
    }
    wasReady.current = detailsReady && !saving;
  }, [saving, detailsLoading, detailsReady, detailsError]);

  function dismiss() {
    if (savingRef.current) return;
    secretRef.current = "";
    setSecret("");
    onClose();
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (savingRef.current || !detailsReady) return;
    setError("");
    if (!issuer.trim() || !name.trim() || (!account && !secretRef.current.trim()) || (kind === "totp" && (!Number.isInteger(period) || period < 1 || period > 86400))) {
      setError(t.required);
      return;
    }
    const payload: AccountInput = {
      issuer: issuer.trim(), account_name: name.trim(), secret: secretRef.current.trim() || null,
      algorithm, digits, kind, period_secs: period, t0, hotp_counter: counter,
      category_id: category || null, tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean), notes, favorite,
    };
    savingRef.current = true;
    secretRef.current = "";
    setSecret("");
    setSaving(true);
    try {
      await onSave(payload);
    } catch {
      setError(t.error);
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  }

  return createPortal(<div className="dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) dismiss(); }}>
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="account-dialog-title" tabIndex={-1} className="dialog-panel" onKeyDown={(event) => trapDialogFocus(event, dismiss)}>
      <div className="flex items-start justify-between gap-4"><div><h2 id="account-dialog-title" className="text-lg font-semibold">{account ? t.edit : t.add}</h2><p className="mt-1 text-sm text-[var(--muted)]">{t.secretPrivacy}</p></div><button type="button" className="icon-button" onClick={dismiss} aria-label={t.cancel} disabled={saving}>×</button></div>
      <form onSubmit={submit} aria-busy={saving || detailsLoading}>
        {detailsLoading && <p role="status" className="loading-state">{t.loading}</p>}
        {detailsError && <div className="inline-error" role="alert"><p>{t.detailsLoadError}</p><button type="button" className="button button-subtle" onClick={() => retryDetails.current()}>{t.retry}</button></div>}
        <fieldset className="form-grid mt-5 min-w-0 border-0 p-0" disabled={saving || !detailsReady}>
          <label>{t.issuer}<input autoFocus value={issuer} onChange={(event) => setIssuer(event.target.value)} maxLength={128} required /></label>
          <label>{t.accountName}<input value={name} onChange={(event) => setName(event.target.value)} maxLength={256} required /></label>
          <label className="field-wide">{t.secret}<input type="password" autoComplete="new-password" value={secret} placeholder={account ? t.secretKeep : ""} aria-describedby={account ? "secret-keep-hint" : undefined} onChange={(event) => { secretRef.current = event.target.value; setSecret(event.target.value); }} />{account && <span id="secret-keep-hint">{t.secretKeep}</span>}</label>
          <label>{t.algorithm}<select value={algorithm} onChange={(event) => setAlgorithm(event.target.value as OtpAlgorithm)}><option value="SHA1">SHA-1</option><option value="SHA256">SHA-256</option><option value="SHA512">SHA-512</option></select></label>
          <label>{t.digits}<select value={digits} onChange={(event) => setDigits(Number(event.target.value))}><option>6</option><option>7</option><option>8</option></select></label>
          <label>{t.kind}<select value={kind} onChange={(event) => setKind(event.target.value as "totp" | "hotp")}><option value="totp">{t.totp}</option><option value="hotp">{t.hotp}</option></select></label>
          {kind === "totp" ? <><label>{t.period}<input type="number" min={1} max={86400} value={period} onChange={(event) => setPeriod(Number(event.target.value))} /></label><label>{t.t0}<input inputMode="numeric" value={t0} onChange={(event) => setT0(event.target.value)} /></label></> : <label>{t.counter}<input inputMode="numeric" value={counter} onChange={(event) => setCounter(event.target.value)} /></label>}
          <label>{t.category}<select value={category} onChange={(event) => setCategory(event.target.value)}><option value="">{t.uncategorized}</option>{categories.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label>
          <label>{t.tags}<input value={tags} onChange={(event) => setTags(event.target.value)} /></label>
          <label className="field-wide">{t.notes}<textarea rows={3} value={notes} onChange={(event) => setNotes(event.target.value)} /></label>
          <label className="check-field field-wide"><input type="checkbox" checked={favorite} onChange={(event) => setFavorite(event.target.checked)} />{t.favorite}</label>
          {error && <p role="alert" className="field-wide field-error">{error}</p>}
          <div className="field-wide flex justify-end gap-2 border-t border-[var(--line)] pt-4"><button type="button" className="button button-subtle" onClick={dismiss} disabled={saving}>{t.cancel}</button><button className="button button-primary" type="submit" disabled={saving || !detailsReady}>{saving ? t.loading : t.save}</button></div>
        </fieldset>
      </form>
    </div>
  </div>, document.body);
}
