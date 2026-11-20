import { useEffect, useRef, useState, type FormEvent } from "react";
import { backupCommands, type AutoBackupStatus } from "../../lib/backup";
import { getMessages, type Locale } from "../../i18n";
import { backupMessages } from "../../i18n/backup";
import { workspaceMessages } from "../../i18n/workspace";

const MAX_INTERVAL = 8760, MAX_RETENTION = 100;
const STATUS_REFRESH_MS = 30_000;

export function AutoBackupSettings({ locale, onError, notify }: { locale: Locale; onError: (error: unknown) => void; notify: (message: string) => void }) {
  const t = getMessages(locale), b = backupMessages(locale), w = workspaceMessages(locale);
  const localeRef = useRef(locale), onErrorRef = useRef(onError);
  localeRef.current = locale;
  onErrorRef.current = onError;
  const [status, setStatus] = useState<AutoBackupStatus | null>(null);
  const [interval, setInterval] = useState("24"), [retention, setRetention] = useState("7");
  const [password, setPassword] = useState(""), [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false), [attempt, setAttempt] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const live = useRef(true), busyRef = useRef(false), refreshingRef = useRef(false);
  const draftDirty = useRef(false), statusRequest = useRef(0);

  useEffect(() => {
    live.current = true;
    const timer = globalThis.setInterval(() => {
      if (!busyRef.current && !refreshingRef.current) setAttempt((value) => value + 1);
    }, STATUS_REFRESH_MS);
    return () => { live.current = false; globalThis.clearInterval(timer); };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const request = ++statusRequest.current;
    refreshingRef.current = true;
    setRefreshing(true);
    backupCommands.status().then((value) => {
      if (cancelled || request !== statusRequest.current) return;
      setStatus(value);
      setLoadFailed(false);
      if (!draftDirty.current) {
        setInterval(String(value.interval_hours));
        setRetention(String(value.retention));
      }
      if (value.paused_locked) {
        setPassword(""); setConfirm("");
        onErrorRef.current({ code: "vault_locked" });
      }
    }).catch((reason) => {
      if (cancelled || request !== statusRequest.current) return;
      if ((reason as { code?: string })?.code === "vault_locked") onErrorRef.current(reason);
      else setLoadFailed(true);
    }).finally(() => {
      if (cancelled) return;
      refreshingRef.current = false;
      setRefreshing(false);
    });
    return () => { cancelled = true; };
  }, [attempt]);

  function bounded(value: string, max: number): number | null {
    const parsed = Number(value);
    return value.trim() !== "" && Number.isInteger(parsed) && parsed >= 1 && parsed <= max ? parsed : null;
  }

  async function configure(enabled: boolean) {
    if (busyRef.current) return;
    if (enabled && password !== confirm) { setError(b.passwordMismatch); return; }
    if (!enabled && !status) return;
    const hours = enabled ? bounded(interval, MAX_INTERVAL) : status!.interval_hours;
    const keep = enabled ? bounded(retention, MAX_RETENTION) : status!.retention;
    if (hours === null || keep === null) { setError(t.status.invalid); return; }
    const secret = password;
    ++statusRequest.current;
    busyRef.current = true;
    setPassword(""); setConfirm(""); setError(""); setBusy(true);
    try {
      const value = await backupCommands.configure(enabled, enabled ? secret : "", hours, keep);
      if (live.current && value) {
        setStatus(value);
        setLoadFailed(false);
        draftDirty.current = false;
        setInterval(String(value.interval_hours));
        setRetention(String(value.retention));
        notify(getMessages(localeRef.current).status.saved);
      }
    } catch (reason) { if (live.current) onErrorRef.current(reason); }
    finally { busyRef.current = false; if (live.current) setBusy(false); }
  }

  function submit(event: FormEvent) { event.preventDefault(); void configure(true); }
  function timestamp(value: number | null | undefined) {
    return value == null ? "—" : new Date(value * 1000).toLocaleString(locale);
  }

  const tag = status?.paused_locked ? b.paused : status?.enabled ? b.enabled : status ? b.disabled : loadFailed ? w.autoLoadError : t.pages.loading;
  const locked = busy || !status || status.paused_locked;

  return <div className="settings-group">
    <div className="section-heading"><h3>{b.auto}</h3><span className="capability-tag workspace-wrap">{tag}</span></div>
    <p className="backup-hint workspace-wrap">{b.autoHint}</p>
    {status && <div className="workspace-feedback" aria-busy={refreshing}>
      <p className="workspace-wrap">{b.lastBackup}: {timestamp(status.last_run)}</p>
      <p className="workspace-wrap">{b.lastAttempt}: {timestamp(status.last_attempt)}</p>
      {status.last_error && <p className="workspace-wrap" role="alert">{b.autoFailed}</p>}
      <p className="backup-hint workspace-wrap">{b.healthHint}</p>
      <button type="button" className="button button-subtle" disabled={busy || refreshing} onClick={() => setAttempt((value) => value + 1)}>{b.refresh}</button>
    </div>}
    {loadFailed ? <div className="workspace-feedback">
      <p role="alert">{w.autoLoadError}</p>
      <button type="button" className="button button-subtle" disabled={busy || refreshing} onClick={() => setAttempt((value) => value + 1)}>{w.retry}</button>
    </div> : <form onSubmit={submit} aria-busy={busy}>
      <fieldset className="settings-fields" disabled={locked}>
        <div className="setting-grid">
          <label>{b.interval}<input type="number" inputMode="numeric" min={1} max={MAX_INTERVAL} required value={interval} onChange={(event) => { draftDirty.current = true; setInterval(event.target.value); }} /></label>
          <label>{b.retention}<input type="number" inputMode="numeric" min={1} max={MAX_RETENTION} required value={retention} onChange={(event) => { draftDirty.current = true; setRetention(event.target.value); }} /></label>
          <label>{t.fields.backupPassword}<input type="password" autoComplete="new-password" required value={password} onChange={(event) => setPassword(event.target.value)} /></label>
          <label>{b.passwordConfirm}<input type="password" autoComplete="new-password" required value={confirm} onChange={(event) => setConfirm(event.target.value)} /></label>
        </div>
        {error && <p role="alert">{error}</p>}
        <div className="backup-actions">
          <button type="button" className="button button-subtle" disabled={busy || !status?.enabled} onClick={() => void configure(false)}>{b.disable}</button>
          <button className="button button-primary" disabled={locked}>{busy ? w.saving : b.configure}</button>
        </div>
      </fieldset>
    </form>}
  </div>;
}
