import { useEffect, useRef, useState, type FormEvent } from "react";
import { backupCommands, type AutoBackupStatus } from "../../lib/backup";
import { getMessages, type Locale } from "../../i18n";
import { backupMessages } from "../../i18n/backup";
import { workspaceMessages } from "../../i18n/workspace";

const MAX_INTERVAL = 8760, MAX_RETENTION = 100;

export function AutoBackupSettings({ locale, onError, notify }: { locale: Locale; onError: (error: unknown) => void; notify: (message: string) => void }) {
  const t = getMessages(locale), b = backupMessages(locale), w = workspaceMessages(locale);
  const localeRef = useRef(locale);
  localeRef.current = locale;
  const [status, setStatus] = useState<AutoBackupStatus | null>(null);
  const [interval, setInterval] = useState("24"), [retention, setRetention] = useState("7");
  const [password, setPassword] = useState(""), [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false), [attempt, setAttempt] = useState(0);
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    setLoadFailed(false);
    backupCommands.status().then((value) => {
      if (!live.current) return;
      setStatus(value);
      setInterval(String(value.interval_hours));
      setRetention(String(value.retention));
    }).catch((reason) => {
      if (!live.current) return;
      if ((reason as { code?: string })?.code === "vault_locked") onError(reason);
      else setLoadFailed(true);
    });
    return () => { live.current = false; };
  }, [attempt, onError]);

  function bounded(value: string, max: number): number | null {
    const parsed = Number(value);
    return value.trim() !== "" && Number.isInteger(parsed) && parsed >= 1 && parsed <= max ? parsed : null;
  }

  async function configure(enabled: boolean) {
    if (busy) return;
    if (enabled && password !== confirm) { setError(b.passwordMismatch); return; }
    if (!enabled && !status) return;
    const hours = enabled ? bounded(interval, MAX_INTERVAL) : status!.interval_hours;
    const keep = enabled ? bounded(retention, MAX_RETENTION) : status!.retention;
    if (hours === null || keep === null) { setError(t.status.invalid); return; }
    const secret = password;
    setPassword(""); setConfirm(""); setError(""); setBusy(true);
    try {
      const value = await backupCommands.configure(enabled, enabled ? secret : "", hours, keep);
      if (live.current && value) {
        setStatus(value);
        setInterval(String(value.interval_hours));
        setRetention(String(value.retention));
        notify(getMessages(localeRef.current).status.saved);
      }
    } catch (reason) { if (live.current) onError(reason); }
    finally { if (live.current) setBusy(false); }
  }

  function submit(event: FormEvent) { event.preventDefault(); void configure(true); }

  const tag = status?.paused_locked ? b.paused : status?.enabled ? b.enabled : status ? b.disabled : loadFailed ? w.autoLoadError : t.pages.loading;
  const locked = busy || !status;

  return <div className="settings-group">
    <div className="section-heading"><h3>{b.auto}</h3><span className="capability-tag workspace-wrap">{tag}</span></div>
    <p className="backup-hint workspace-wrap">{b.autoHint}</p>
    {loadFailed ? <div className="workspace-feedback">
      <p role="alert">{w.autoLoadError}</p>
      <button className="button button-subtle" onClick={() => setAttempt((value) => value + 1)}>{w.retry}</button>
    </div> : <form onSubmit={submit} aria-busy={busy}>
      <fieldset className="settings-fields" disabled={locked}>
        <div className="setting-grid">
          <label>{b.interval}<input type="number" inputMode="numeric" min={1} max={MAX_INTERVAL} required value={interval} onChange={(event) => setInterval(event.target.value)} /></label>
          <label>{b.retention}<input type="number" inputMode="numeric" min={1} max={MAX_RETENTION} required value={retention} onChange={(event) => setRetention(event.target.value)} /></label>
          <label>{t.fields.backupPassword}<input type="password" autoComplete="new-password" required value={password} onChange={(event) => setPassword(event.target.value)} /></label>
          <label>{b.passwordConfirm}<input type="password" autoComplete="new-password" required value={confirm} onChange={(event) => setConfirm(event.target.value)} /></label>
        </div>
        {error && <p role="alert">{error}</p>}
        <div className="backup-actions">
          <span className="text-sm text-[var(--muted)] workspace-wrap">{b.lastBackup}: {status?.last_run ? new Date(status.last_run * 1000).toLocaleString(locale) : "—"}</span>
          <button type="button" className="button button-subtle" disabled={busy || !status?.enabled} onClick={() => void configure(false)}>{b.disable}</button>
          <button className="button button-primary" disabled={locked}>{busy ? w.saving : b.configure}</button>
        </div>
      </fieldset>
    </form>}
  </div>;
}
