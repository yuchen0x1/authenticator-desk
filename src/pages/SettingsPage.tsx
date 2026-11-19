import { useRef, useState, type FormEvent } from "react";
import { AutoBackupSettings } from "../components/backup/AutoBackupSettings";
import { DesktopSettings } from "../components/desktop/DesktopSettings";
import { UpdateSettings } from "../components/desktop/UpdateSettings";
import { trapDialogFocus } from "../components/common/dialogFocus";
import { getMessages } from "../i18n";
import { changeMasterPassword } from "../lib/tauri";
import type { AppSettings } from "../lib/types";
import type { WorkspaceProps } from "../App";

export function SettingsPage({ locale, settings, saveSettings, onError, notify }: WorkspaceProps) {
  const t = getMessages(locale);
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const returnFocus = useRef<HTMLElement | null>(null);

  function closePassword() {
    setPasswordOpen(false);
    setCurrent("");
    setNext("");
    setConfirm("");
    setError("");
    returnFocus.current?.focus();
  }

  function change<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    setDraft((old) => ({ ...old, [key]: value }));
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    try {
      await saveSettings(draft);
    } catch (reason) {
      onError(reason);
    }
  }

  async function updatePassword(event: FormEvent) {
    event.preventDefault();
    if (next.length < 12) {
      setError(t.status.weakPassword);
      return;
    }
    if (next !== confirm) {
      setError(t.status.mismatch);
      return;
    }

    const oldPassword = current;
    const newPassword = next;
    setCurrent("");
    setNext("");
    setConfirm("");
    setError("");
    try {
      await changeMasterPassword(oldPassword, newPassword);
      closePassword();
      notify(t.status.saved);
    } catch (reason) {
      onError(reason);
    }
  }

  return <section>
    <div className="page-heading"><div><h2>{t.settings.title}</h2></div></div>
    <form onSubmit={save} className="settings-form">
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
          <label>{t.fields.idleTimeout}<input type="number" min={0} max={86400} value={draft.idle_timeout_secs} onChange={(event) => change("idle_timeout_secs", Number(event.target.value))} /></label>
          <label>{t.fields.clipboardTtl}<input type="number" min={0} max={3600} value={draft.clipboard_ttl_secs} onChange={(event) => change("clipboard_ttl_secs", Number(event.target.value))} /></label>
          <label>{t.settings.defaultDigits}
            <select value={draft.default_digits} onChange={(event) => change("default_digits", Number(event.target.value))}>
              <option value={6}>6</option><option value={7}>7</option><option value={8}>8</option>
            </select>
          </label>
          <label>{t.settings.defaultPeriod}<input type="number" min={1} max={3600} value={draft.default_period_secs} onChange={(event) => change("default_period_secs", Number(event.target.value))} /></label>
        </div>
        <Capability label={t.settings.windowsHello} detail={t.settings.windowsHelloDetail} />
        <button type="button" className="button button-subtle mt-4" onClick={() => {
          returnFocus.current = document.activeElement as HTMLElement;
          setPasswordOpen(true);
        }}>{t.actions.changePassword}</button>
      </div>
      <div className="flex justify-end border-t border-[var(--line)] pt-5">
        <button className="button button-primary">{t.actions.save}</button>
      </div>
    </form>
    <UpdateSettings locale={locale} />
    <DesktopSettings locale={locale} onError={onError} notify={notify} />
    <AutoBackupSettings locale={locale} onError={onError} notify={notify} />
    {passwordOpen && <div className="dialog-backdrop">
      <div role="dialog" aria-modal="true" aria-labelledby="password-title" className="dialog-panel dialog-small" onKeyDown={(event) => trapDialogFocus(event, closePassword)}>
        <h2 id="password-title">{t.actions.changePassword}</h2>
        <form className="mt-5 grid gap-4" onSubmit={updatePassword}>
          <label>{t.fields.currentPassword}<input type="password" autoComplete="current-password" value={current} onChange={(event) => setCurrent(event.target.value)} autoFocus required /></label>
          <label>{t.fields.newPassword}<input type="password" autoComplete="new-password" value={next} onChange={(event) => setNext(event.target.value)} required minLength={12} /></label>
          <label>{t.shell.confirmPassword}<input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} required /></label>
          {error && <p role="alert" className="text-sm text-rose-300">{error}</p>}
          <div className="flex justify-end gap-2">
            <button type="button" className="button button-subtle" onClick={closePassword}>{t.actions.cancel}</button>
            <button className="button button-primary">{t.actions.save}</button>
          </div>
        </form>
      </div>
    </div>}
  </section>;
}

function Toggle({ checked, label, onChange }: { checked: boolean; label: string; onChange: (value: boolean) => void }) {
  return <label className="toggle-row"><span>{label}</span><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /></label>;
}

function Capability({ label, detail }: { label: string; detail: string }) {
  return <div className="capability-inline"><span>{label}</span><span className="text-[var(--muted)]">{detail}</span></div>;
}
