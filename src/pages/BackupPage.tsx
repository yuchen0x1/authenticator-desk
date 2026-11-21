import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { getMessages } from "../i18n";
import { backupMessages } from "../i18n/backup";
import { getPreviewUi } from "../i18n/previewUi";
import { backupCommands, type BackupFileView, type RestoreMode, type RestorePreview } from "../lib/backup";
import { AutoBackupSettings } from "../components/backup/AutoBackupSettings";
import type { WorkspaceProps } from "../App";

type Operation = "create" | "restore";
type BusyAction = "create" | "preview" | "apply" | "browse" | null;

export function BackupPage({ locale, onError, notify, refresh }: WorkspaceProps) {
  const t = getMessages(locale);
  const b = backupMessages(locale);
  const ui = getPreviewUi(locale);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [operation, setOperation] = useState<Operation>("create");
  const [mode, setMode] = useState<RestoreMode>("merge");
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [error, setError] = useState("");
  const [files, setFiles] = useState<BackupFileView[] | null>(null);
  const busy = busyAction !== null;
  const live = useRef(true);
  const working = useRef(false);
  const pendingPreview = useRef<string | null>(null);
  const generation = useRef(0);
  const previewRegion = useRef<HTMLElement | null>(null);
  const previewSubmit = useRef<HTMLButtonElement | null>(null);
  const previewTrigger = useRef<HTMLElement | null>(null);
  const focusWhenIdle = useRef(false);

  function cancelPreview(id: string) {
    void backupCommands.cancel(id).catch(() => {
      // The preview may already have been replaced, expired, or cleared on lock.
    });
  }

  function clearPreview() {
    const id = pendingPreview.current;
    pendingPreview.current = null;
    setPreview(null);
    setSelected([]);
    setConfirmed(false);
    if (id) cancelPreview(id);
  }

  function focusTrigger() {
    const target = previewTrigger.current;
    if (target?.isConnected && !target.hasAttribute("disabled")) target.focus();
  }

  function focusTriggerWhenIdle() {
    if (working.current) focusWhenIdle.current = true;
    else focusTrigger();
  }

  useLayoutEffect(() => {
    if (preview) previewRegion.current?.focus();
  }, [preview?.preview_id]);

  useLayoutEffect(() => {
    if (busyAction !== null || !focusWhenIdle.current) return;
    focusWhenIdle.current = false;
    focusTrigger();
  }, [busyAction]);

  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
      generation.current++;
      const id = pendingPreview.current;
      pendingPreview.current = null;
      if (id) cancelPreview(id);
    };
  }, []);

  function switchOperation(value: Operation) {
    generation.current++;
    clearPreview();
    setOperation(value);
    setPassword("");
    setConfirm("");
    setError("");
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (working.current) return;
    const submitter = (event.nativeEvent as SubmitEvent).submitter;
    previewTrigger.current = submitter instanceof HTMLElement ? submitter : previewSubmit.current;
    if (operation === "create" && password !== confirm) {
      setError(b.passwordMismatch);
      return;
    }

    const secret = password;
    const token = ++generation.current;
    working.current = true;
    setPassword("");
    setConfirm("");
    setError("");
    setBusyAction(operation === "create" ? "create" : "preview");

    try {
      if (operation === "create") {
        clearPreview();
        const value = await backupCommands.create(secret);
        if (live.current && token === generation.current && value) {
          notify(`${b.created}: ${value.account_count}`);
        }
      } else {
        // The native picker returns null before the backend clears its pending
        // slot. Keep the old review until a real replacement arrives or the
        // preview operation fails.
        const value = await backupCommands.preview(secret, mode);
        if (!value) {
          if (live.current && token === generation.current) focusTriggerWhenIdle();
          return;
        }
        if (!live.current || token !== generation.current) {
          cancelPreview(value.preview_id);
          return;
        }
        const previous = pendingPreview.current;
        pendingPreview.current = value.preview_id;
        if (previous && previous !== value.preview_id) cancelPreview(previous);
        setPreview(value);
        setSelected(value.items.filter((item) => item.status === "new").map((item) => item.index));
        setConfirmed(false);
      }
    } catch (reason) {
      if (live.current && token === generation.current) {
        // The backup service clears its pending slot before decoding a chosen
        // file, so a failed replacement must also disappear from this page.
        clearPreview();
        focusTriggerWhenIdle();
        onError(reason);
      }
    } finally {
      working.current = false;
      if (live.current && token === generation.current) setBusyAction(null);
    }
  }

  async function apply() {
    const current = preview;
    const selection = selected;
    const replaceConfirmed = confirmed;
    if (!current || (current.mode === "replace" && !replaceConfirmed) || working.current) return;
    working.current = true;
    setBusyAction("apply");
    let result: Awaited<ReturnType<typeof backupCommands.restore>>;
    try {
      result = await backupCommands.restore(current.preview_id, selection, replaceConfirmed);
      pendingPreview.current = null;
      if (live.current) {
        setPreview(null);
        setSelected([]);
        setConfirmed(false);
        focusTriggerWhenIdle();
        notify(`${b.restored}: ${result.restored_accounts}`);
      }
    } catch (reason) {
      if (live.current) {
        clearPreview();
        focusTriggerWhenIdle();
        onError(reason);
      }
      return;
    } finally {
      working.current = false;
      if (live.current) setBusyAction(null);
    }
    if (result.mode === "merge") {
      // Restore succeeded already: do not turn a refresh failure into a failed
      // restore message or recreate a preview that the backend consumed.
      try { await refresh(); }
      catch (reason) { if (live.current) onError(reason); }
    }
  }

  async function browse() {
    if (working.current) return;
    working.current = true;
    setBusyAction("browse");
    try {
      const value = await backupCommands.list();
      if (live.current && value) setFiles(value);
    } catch (reason) {
      if (live.current) onError(reason);
    } finally {
      working.current = false;
      if (live.current) setBusyAction(null);
    }
  }

  return <section>
    <div className="page-heading"><div><h2>{t.pages.backup}</h2><p>{b.hint}</p></div></div>
    <div className="settings-group">
      <div className="segmented backup-tabs">
        <button type="button" aria-pressed={operation === "create"} disabled={busy} onClick={() => switchOperation("create")}>{b.create}</button>
        <button type="button" aria-pressed={operation === "restore"} disabled={busy} onClick={() => switchOperation("restore")}>{b.restore}</button>
      </div>
      <form onSubmit={submit} aria-busy={busy}>
        <div className="setting-grid">
          <label>{t.fields.backupPassword}
            <input type="password" autoComplete={operation === "create" ? "new-password" : "current-password"} value={password} onChange={(event) => setPassword(event.target.value)} required disabled={busy} />
          </label>
          {operation === "create" ? <label>{b.passwordConfirm}
            <input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} required disabled={busy} />
          </label> : <label>{b.restore}
            <select disabled={busy} value={mode} onChange={(event) => {
              generation.current++;
              clearPreview();
              setMode(event.target.value as RestoreMode);
            }}>
              <option value="merge">{b.merge}</option>
              <option value="replace">{b.replace}</option>
            </select>
          </label>}
        </div>
        {operation === "restore" && mode === "replace" && <p className="backup-warning">{b.replaceWarning}</p>}
        {error && <p role="alert">{error}</p>}
        <div className="backup-actions">
          <button ref={previewSubmit} type="submit" className="button button-primary" disabled={busy}>
            {busyAction === "create" ? ui.backupCreateBusy : busyAction === "preview" ? ui.backupPreviewBusy : operation === "create" ? b.create : b.preview}
          </button>
        </div>
      </form>
    </div>
    {preview && <section ref={previewRegion} className="preview-section" tabIndex={-1} role="region" aria-labelledby="backup-preview-heading" aria-busy={busy}>
      <div className="visually-hidden" role="status" aria-live="polite">{ui.backupPreviewReady(preview.account_count)}</div>
      <div className="section-heading">
        <h3 id="backup-preview-heading">{b.restore}</h3>
        <span>{preview.account_count} · {t.navigation.categories}: {preview.category_count}</span>
      </div>
      <div className="preview-list" role="list" aria-label={b.restore}>
      {preview.items.map((item) => <label key={item.index} className="preview-row" role="listitem">
        <input type="checkbox" disabled={busy || item.status !== "new"} checked={selected.includes(item.index)} onChange={(event) => setSelected((old) => event.target.checked ? [...old, item.index] : old.filter((index) => index !== item.index))} />
        <span className="min-w-0 flex-1">
          <strong className="block truncate" title={item.issuer}>{item.issuer}</strong>
          <span className="block min-w-0 truncate text-sm text-[var(--muted)]" title={item.account_name}>{item.account_name}</span>
        </span>
        <span className={`import-status status-${item.status}`}>{item.status === "new" ? t.status.newItem : t.status.duplicate}</span>
      </label>)}
      </div>
      {preview.mode === "replace" && <div className="backup-warning">
        <p id="backup-replace-warning">{b.replaceWarning}</p>
        <p>{b.removeCount}: <strong>{preview.existing_count}</strong></p>
        <p>{b.restoreCount}: <strong>{selected.length}</strong></p>
        <p>{b.replacePolicy}</p>
        <label className="backup-confirm">
          <input type="checkbox" aria-describedby="backup-replace-warning" disabled={busy} checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} />
          {b.confirmReplace}
        </label>
      </div>}
      <div className="backup-actions">
        <button type="button" className="button button-subtle" disabled={busy} onClick={() => {
          generation.current++;
          clearPreview();
          focusTriggerWhenIdle();
        }}>{t.actions.cancel}</button>
        <button type="button" className="button button-primary" disabled={busy || !selected.length || (preview.mode === "replace" && !confirmed)} onClick={() => void apply()}>
          {busyAction === "apply" ? ui.backupApplyBusy : `${b.apply} (${selected.length})`}
        </button>
      </div>
    </section>}
    <AutoBackupSettings locale={locale} onError={onError} notify={notify} />
    <div className="settings-group">
      <div className="section-heading"><h3>{b.browse}</h3><button type="button" className="button button-subtle" disabled={busy} onClick={() => void browse()}>{busyAction === "browse" ? ui.backupBrowseBusy : b.browse}</button></div>
      {files && (!files.length ? <p className="backup-hint">{b.empty}</p> : <div className="backup-table-wrap"><table className="backup-table"><thead><tr><th>{b.file}</th><th>{b.size}</th><th>{b.modified}</th><th>{t.fields.kind}</th></tr></thead><tbody>{files.map((file) => <tr key={file.name}><td>{file.name}</td><td>{(file.bytes / 1024).toFixed(1)} KiB</td><td>{file.modified_at ? new Date(Number(file.modified_at) * 1000).toLocaleString(locale) : "—"}</td><td>{file.automatic ? b.automatic : b.manual}</td></tr>)}</tbody></table></div>)}
    </div>
  </section>;
}
