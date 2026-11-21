import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import { AutoBackupSettings } from "../components/backup/AutoBackupSettings";
import { DesktopSettings } from "../components/desktop/DesktopSettings";
import { UpdateSettings } from "../components/desktop/UpdateSettings";
import { focusDialog, trapDialogFocus } from "../components/common/dialogFocus";
import { useModalIsolation } from "../components/common/useModalIsolation";
import { getMessages } from "../i18n";
import { timerMessages } from "../i18n/timers";
import { changeMasterPassword } from "../lib/tauri";
import type { AppSettings } from "../lib/types";
import type { WorkspaceProps } from "../App";

// Keep these bounds in sync with AppSettings::validate in accounts/model.rs.
const idleMin = 30;
const idleMax = 86_400;
const clipboardMax = 300;
const periodMax = 86_400;

function parseSeconds(value: string, min: number, max: number) {
  if (!/^\d+$/.test(value)) return undefined;
  const seconds = Number(value);
  return seconds >= min && seconds <= max ? seconds : undefined;
}

export function SettingsPage({ locale, settings, saveSettings, onError, notify }: WorkspaceProps) {
  const t = getMessages(locale), timers = timerMessages(locale);
  const [idleDisabled,setIdleDisabled] = useState(settings.idle_timeout_secs === 0);
  const [clipboardDisabled,setClipboardDisabled] = useState(settings.clipboard_ttl_secs === 0);
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [idleTimeout, setIdleTimeout] = useState(() => String(settings.idle_timeout_secs));
  const [clipboardTtl, setClipboardTtl] = useState(() => String(settings.clipboard_ttl_secs));
  const [defaultPeriod, setDefaultPeriod] = useState(() => String(settings.default_period_secs));
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [passwordSaving, setPasswordSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const savingRef = useRef(false);
  const passwordSavingRef = useRef(false);
  const live = useRef(true);
  const dirtyFields = useRef(new Set<keyof AppSettings>());
  const receivedSettings = useRef(settings);
  const latestSettings = useRef(settings);
  const passwordDialog = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  // Keep an acknowledged save as the baseline until new props arrive. Newer
  // settings props always win over a completed request's stale snapshot.
  if (receivedSettings.current !== settings) {
    receivedSettings.current = settings;
    latestSettings.current = settings;
  }

  useEffect(() => {
    live.current = true;
    return () => { live.current = false; };
  }, []);
  useModalIsolation(passwordOpen, passwordDialog, returnFocus);
  useLayoutEffect(() => {
    const dialog = passwordDialog.current;
    const active = document.activeElement;
    if (passwordOpen && dialog && (!dialog.contains(active) || active === dialog || active?.matches(":disabled"))) focusDialog(dialog);
  }, [passwordOpen, passwordSaving]);

  function syncDraft(value: AppSettings) {
    setDraft((current) => {
      const next = { ...value };
      for (const key of dirtyFields.current) next[key] = current[key] as never;
      return next;
    });
    if (!dirtyFields.current.has("idle_timeout_secs")) {
      setIdleDisabled(value.idle_timeout_secs === 0);
      setIdleTimeout(String(value.idle_timeout_secs));
    }
    if (!dirtyFields.current.has("clipboard_ttl_secs")) {
      setClipboardDisabled(value.clipboard_ttl_secs === 0);
      setClipboardTtl(String(value.clipboard_ttl_secs));
    }
    if (!dirtyFields.current.has("default_period_secs")) setDefaultPeriod(String(value.default_period_secs));
  }
  useEffect(() => { syncDraft(latestSettings.current); }, [settings]);

  function closePassword(force = false) {
    if (passwordSavingRef.current && !force) return;
    setPasswordOpen(false);
    setCurrent("");
    setNext("");
    setConfirm("");
    setError("");
  }

  function markDirty(key: keyof AppSettings) {
    if (savingRef.current || passwordSavingRef.current) return false;
    dirtyFields.current.add(key);
    setSaveError("");
    return true;
  }

  function change<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (markDirty(key)) setDraft((old) => ({ ...old, [key]: value }));
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (savingRef.current || passwordSavingRef.current || passwordOpen) return;
    const baseline = latestSettings.current;
    const fields = new Set(dirtyFields.current);
    const idle = fields.has("idle_timeout_secs") ? (idleDisabled ? 0 : parseSeconds(idleTimeout, idleMin, idleMax)) : baseline.idle_timeout_secs;
    const clipboard = fields.has("clipboard_ttl_secs") ? (clipboardDisabled ? 0 : parseSeconds(clipboardTtl, 1, clipboardMax)) : baseline.clipboard_ttl_secs;
    const period = fields.has("default_period_secs") ? parseSeconds(defaultPeriod, 1, periodMax) : baseline.default_period_secs;
    if (idle === undefined || clipboard === undefined || period === undefined) {
      setSaveError("");
      onError({ code: "invalid_input" });
      return;
    }
    const value = { ...baseline };
    for (const key of fields) value[key] = draft[key] as never;
    value.idle_timeout_secs = idle;
    value.clipboard_ttl_secs = clipboard;
    value.default_period_secs = period;
    savingRef.current = true;
    setSaving(true);
    setSaveError("");
    try {
      await saveSettings(value);
      if (!live.current) return;
      for (const key of fields) dirtyFields.current.delete(key);
      if (latestSettings.current === baseline) latestSettings.current = value;
      syncDraft(latestSettings.current);
    } catch (reason) {
      if (live.current) {
        // vault_locked must still surface through onError so the app clears the
        // workspace; other failures stay in this retryable local alert only.
        if ((reason as { code?: string })?.code === "vault_locked") onError(reason);
        else setSaveError(t.status.error);
      }
    } finally {
      savingRef.current = false;
      if (live.current) setSaving(false);
    }
  }

  async function updatePassword(event: FormEvent) {
    event.preventDefault();
    if (passwordSavingRef.current || savingRef.current || !passwordOpen) return;
    if (next.length < 12) {
      setError(t.status.weakPassword);
      return;
    }
    if (next !== confirm) {
      setError(t.status.mismatch);
      return;
    }
    if (!current) {
      setError(t.status.invalid);
      return;
    }

    const oldPassword = current;
    const newPassword = next;
    passwordSavingRef.current = true;
    setCurrent("");
    setNext("");
    setConfirm("");
    setError("");
    setPasswordSaving(true);
    try {
      await changeMasterPassword(oldPassword, newPassword);
      if (live.current) {
        closePassword(true);
        notify(t.status.saved);
      }
    } catch (reason) {
      if (live.current) {
        if ((reason as { code?: string })?.code === "vault_locked") onError(reason);
        else setError(t.status.error);
      }
    } finally {
      passwordSavingRef.current = false;
      if (live.current) setPasswordSaving(false);
    }
  }

  return <section>
    <div className="page-heading"><div><h2>{t.settings.title}</h2></div></div>
    <form onSubmit={save} className="settings-form" aria-busy={saving}>
      <fieldset className="settings-form-fieldset" disabled={saving}>
      <div className="settings-group">
        <h3>{t.settings.appearance}</h3>
        <div className="setting-grid">
          <label>{t.fields.locale}
            <select value={draft.locale} onChange={(event) => change("locale", event.target.value)}>
              <option value="zh-CN">简体中文</option>
              <option value="zh-TW">繁體中文</option>
              <option value="en">English</option>
              <option value="ja">日本語</option>
            </select>
          </label>
          <label>{t.fields.theme}
            <select value={draft.theme} onChange={(event) => change("theme", event.target.value)}>
              <option value="dark">{t.fields.dark}</option>
              <option value="light">{t.fields.light}</option>
            </select>
          </label>
          <fieldset>
            <legend>{t.settings.view}</legend>
            <div className="segmented">
              <button type="button" aria-pressed={draft.view_mode !== "list"} onClick={() => change("view_mode", "cards")}>{t.settings.card}</button>
              <button type="button" aria-pressed={draft.view_mode === "list"} onClick={() => change("view_mode", "list")}>{t.settings.list}</button>
            </div>
          </fieldset>
        </div>
        <Toggle checked={draft.animations} label={t.settings.animations} onChange={(value) => change("animations", value)} />
        <Toggle checked={draft.compact} label={t.settings.compact} onChange={(value) => change("compact", value)} />
        <Toggle checked={draft.hide_codes} label={t.settings.hideCodes} onChange={(value) => change("hide_codes", value)} />
      </div>

      <div className="settings-group">
        <h3>{t.settings.security}</h3>
        <div className="setting-grid">
          <div>
            <label>{t.fields.idleTimeout}
              <input type="number" min={idleMin} max={idleMax} step={1} required disabled={idleDisabled} value={idleTimeout} onChange={(event) => { if (markDirty("idle_timeout_secs")) setIdleTimeout(event.target.value); }} aria-describedby="idle-timeout-range" />
            </label>
            <p id="idle-timeout-range" className="mt-1 text-xs text-[var(--muted)]">{idleMin}–{idleMax}</p>
            <Toggle checked={idleDisabled} label={timers.disableIdle} onChange={(value) => { if (!markDirty("idle_timeout_secs")) return; setIdleDisabled(value); if (!value && parseSeconds(idleTimeout, idleMin, idleMax) === undefined) setIdleTimeout("900"); }} />
          </div>
          <div>
            <label>{t.fields.clipboardTtl}
              <input type="number" min={1} max={clipboardMax} step={1} required disabled={clipboardDisabled} value={clipboardTtl} onChange={(event) => { if (markDirty("clipboard_ttl_secs")) setClipboardTtl(event.target.value); }} aria-describedby="clipboard-ttl-range" />
            </label>
            <p id="clipboard-ttl-range" className="mt-1 text-xs text-[var(--muted)]">1–{clipboardMax}</p>
            <Toggle checked={clipboardDisabled} label={timers.disableClipboard} onChange={(value) => { if (!markDirty("clipboard_ttl_secs")) return; setClipboardDisabled(value); if (!value && parseSeconds(clipboardTtl, 1, clipboardMax) === undefined) setClipboardTtl("30"); }} />
          </div>
          <label>{t.settings.defaultDigits}
            <select value={draft.default_digits} onChange={(event) => change("default_digits", Number(event.target.value))}>
              <option value={6}>6</option><option value={7}>7</option><option value={8}>8</option>
            </select>
          </label>
          <label>{t.settings.defaultPeriod}<input type="number" min={1} max={periodMax} step={1} required value={defaultPeriod} onChange={(event) => { if (markDirty("default_period_secs")) setDefaultPeriod(event.target.value); }} /></label>
        </div>
        <Capability label={t.settings.windowsHello} detail={t.settings.windowsHelloDetail} />
        <button type="button" className="button button-subtle mt-4" onClick={(event) => {
          if (savingRef.current || passwordSavingRef.current) return;
          returnFocus.current = event.currentTarget;
          setPasswordOpen(true);
        }}>{t.actions.changePassword}</button>
      </div>
      {saveError && <p role="alert" className="field-error">{saveError}</p>}
      <div className="flex justify-end border-t border-[var(--line)] pt-5">
        <button className="button button-primary" disabled={saving}>{saving ? t.pages.loading : t.actions.save}</button>
      </div>
      </fieldset>
    </form>
    <UpdateSettings locale={locale} />
    <DesktopSettings locale={locale} onError={onError} notify={notify} />
    <AutoBackupSettings locale={locale} onError={onError} notify={notify} />
    {passwordOpen && createPortal(<div className="dialog-backdrop">
      <div ref={passwordDialog} role="dialog" aria-modal="true" aria-labelledby="password-title" tabIndex={-1} className="dialog-panel dialog-small" onKeyDown={(event) => trapDialogFocus(event, closePassword)}>
        <h2 id="password-title">{t.actions.changePassword}</h2>
        <form className="mt-5" onSubmit={updatePassword} aria-busy={passwordSaving}>
          <fieldset className="grid gap-4 border-0 p-0" disabled={passwordSaving}>
            <label>{t.fields.currentPassword}<input type="password" autoComplete="current-password" value={current} onChange={(event) => setCurrent(event.target.value)} autoFocus required /></label>
            <label>{t.fields.newPassword}<input type="password" autoComplete="new-password" value={next} onChange={(event) => setNext(event.target.value)} required minLength={12} /></label>
            <label>{t.shell.confirmPassword}<input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} required /></label>
            {error && <p role="alert" className="field-error">{error}</p>}
            <div className="flex justify-end gap-2">
              <button type="button" className="button button-subtle" onClick={() => closePassword()}>{t.actions.cancel}</button>
              <button className="button button-primary">{passwordSaving ? t.pages.loading : t.actions.save}</button>
            </div>
          </fieldset>
        </form>
      </div>
    </div>, document.body)}
  </section>;
}

function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><span>{label}</span><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /></label>;
}

function Capability({ label, detail }: { label: string; detail: string }) {
  return <div className="capability-inline"><span>{label}</span><span className="text-[var(--muted)]">{detail}</span></div>;
}
