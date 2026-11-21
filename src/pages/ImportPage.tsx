import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { desktopCommands, importCommands } from "../lib/tauri";
import type { ImportPreview } from "../lib/types";
import { getMessages } from "../i18n";
import { getPreviewUi } from "../i18n/previewUi";
import { importErrorLabel, importFormatLabel } from "../i18n/importUi";
import type { WorkspaceProps } from "../App";

async function cancelPreview(id: string) {
  try {
    await importCommands.cancel(id);
  } catch {
    // The preview may already have been replaced, expired, or cleared on lock.
  }
}

type BusyAction = "text" | "file" | "qr" | "commit" | null;
type PreviewAction = Exclude<BusyAction, "commit" | null>;

export function ImportPage({ locale, refresh, notify, onError }: WorkspaceProps) {
  const t = getMessages(locale);
  const ui = getPreviewUi(locale);
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  // The backend takes the preview id before validating, so any commit failure
  // (expired slot, conflict, revision change) kills the review permanently.
  const [expired, setExpired] = useState(false);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const busy = busyAction !== null;
  const working = useRef(false);
  const mounted = useRef(true);
  const pendingPreview = useRef<string | null>(null);
  const previewRegion = useRef<HTMLElement | null>(null);
  const previewSubmit = useRef<HTMLButtonElement | null>(null);
  const previewTrigger = useRef<HTMLElement | null>(null);
  const focusWhenIdle = useRef(false);

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
    mounted.current = true;
    return () => {
      mounted.current = false;
      const id = pendingPreview.current;
      pendingPreview.current = null;
      if (id) void cancelPreview(id);
    };
  }, []);

  async function run(action: Exclude<BusyAction, null>, operation: () => Promise<void>) {
    if (working.current) return;
    working.current = true;
    setBusyAction(action);
    try {
      await operation();
    } catch (error) {
      if (mounted.current) onError(error);
    } finally {
      working.current = false;
      if (mounted.current) setBusyAction(null);
    }
  }

  function inspect(action: PreviewAction, load: () => Promise<ImportPreview | null>) {
    void run(action, async () => {
      const value = await load();
      if (!value) {
        // Native cancellation happens before the backend replaces its slot.
        // Keep the current preview and return focus after the picker closes.
        focusTriggerWhenIdle();
        return;
      }
      if (!mounted.current) {
        void cancelPreview(value.preview_id);
        return;
      }
      const previous = pendingPreview.current;
      pendingPreview.current = value.preview_id;
      if (previous && previous !== value.preview_id) void cancelPreview(previous);
      setPreview(value);
      setSelected(value.items.filter((item) => item.status === "new").map((item) => item.index));
      setText("");
      setExpired(false);
    });
  }

  function inspectText(event: FormEvent) {
    event.preventDefault();
    const submitter = (event.nativeEvent as SubmitEvent).submitter;
    previewTrigger.current = submitter instanceof HTMLElement ? submitter : previewSubmit.current;
    if (!text.trim()) {
      onError({ code: "invalid_input" });
      return;
    }
    inspect("text", () => importCommands.preview(text));
  }

  function dismiss() {
    const id = pendingPreview.current;
    pendingPreview.current = null;
    setPreview(null);
    setSelected([]);
    setExpired(false);
    if (id) void cancelPreview(id);
    focusTriggerWhenIdle();
  }

  function commit() {
    if (!preview || !selected.length || working.current) return;
    // Mirrors run(), but a failure must also drop the review: the backend takes
    // the preview id before it validates the selection or the vault revision, so
    // offering the same id again would only repeat the same conflict.
    working.current = true;
    setBusyAction("commit");
    void (async () => {
      let count: number;
      try {
        count = await importCommands.commit(preview.preview_id, selected);
        pendingPreview.current = null;
        if (mounted.current) {
          setPreview(null);
          setSelected([]);
          setExpired(false);
          focusTriggerWhenIdle();
        }
      } catch (error) {
        pendingPreview.current = null;
        if (mounted.current) {
          setPreview(null);
          setSelected([]);
          setExpired(true);
          focusTriggerWhenIdle();
        }
        if (mounted.current && (error as { code?: string })?.code === "vault_locked") onError(error);
        return;
      } finally {
        working.current = false;
        if (mounted.current) setBusyAction(null);
      }
      if (mounted.current) notify(`${t.status.imported}: ${count}`);
      // Update the shared account list even if the user switched pages while
      // committing. A refresh failure cannot undo the completed import.
      try { await refresh(); }
      catch (error) { if (mounted.current || (error as { code?: string })?.code === "vault_locked") onError(error); }
    })();
  }

  return <section>
    <div className="page-heading"><div><h2>{t.pages.import}</h2><p>{t.pages.emptyHint}</p></div></div>
    <form onSubmit={inspectText} className="import-form" aria-busy={busy}>
      <label>{t.fields.secret}
        <textarea rows={6} spellCheck={false} autoComplete="off" value={text} onChange={(event) => setText(event.target.value)} placeholder="otpauth://totp/Service:account?..." required disabled={busy} />
      </label>
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" className="button button-subtle" onClick={(event) => {
          previewTrigger.current = event.currentTarget;
          inspect("file", desktopCommands.pickImportFile);
        }} disabled={busy}>{busyAction === "file" ? ui.importFileBusy : t.actions.chooseFile}</button>
        <button type="button" className="button button-subtle" onClick={(event) => {
          previewTrigger.current = event.currentTarget;
          inspect("qr", desktopCommands.pickQrImage);
        }} disabled={busy}>{busyAction === "qr" ? ui.importQrBusy : t.actions.chooseQr}</button>
        <p className="text-xs text-[var(--muted)]">{t.pages.accountsHint}</p>
        <button ref={previewSubmit} onClick={(event) => { previewTrigger.current = event.currentTarget; }} className="button button-primary ml-auto" disabled={busy} type="submit">{busyAction === "text" ? ui.importPreviewBusy : t.actions.preview}</button>
      </div>
    </form>
    {expired && !preview && <p className="workspace-feedback" role="status">{ui.importPreviewExpired}</p>}
    {preview && <section ref={previewRegion} className="preview-section" tabIndex={-1} role="region" aria-labelledby="import-preview-heading" aria-busy={busy}>
      <div className="visually-hidden" role="status" aria-live="polite">{ui.importPreviewReady(preview.items.length)}</div>
      <div className="section-heading">
        <h3 id="import-preview-heading">{t.actions.preview}</h3>
        <span className="text-xs text-[var(--muted)]">{t.status.formatLabel}: <span title={preview.format}>{importFormatLabel(locale, preview.format)}</span></span>
        <span>{preview.items.length}</span>
      </div>
      <div className="preview-list" role="list" aria-label={t.pages.import}>
        {preview.items.map((item) => <label key={item.index} className="preview-row" role="listitem">
        <input type="checkbox" disabled={busy || item.status !== "new"} checked={selected.includes(item.index)} onChange={(event) => {
          const checked = event.target.checked;
          setSelected((old) => checked ? [...old, item.index] : old.filter((index) => index !== item.index));
        }} />
        <span className="min-w-0 flex-1">
          <strong className="block truncate" title={item.issuer ?? undefined}>{item.issuer ?? t.status.invalid}</strong>
          {item.account_name != null && <span className="block min-w-0 truncate text-sm text-[var(--muted)]" title={item.account_name}>{item.account_name}</span>}
          {item.error_code && <span className="block text-sm text-[var(--muted)]">{importErrorLabel(locale, item.error_code)}</span>}
        </span>
        <span className={`import-status status-${item.status}`}>{item.status === "new" ? t.status.newItem : item.status === "duplicate" ? t.status.duplicate : t.status.invalid}</span>
      </label>)}
      </div>
      <div className="flex justify-end gap-2 border-t border-[var(--line)] pt-4">
        <button type="button" className="button button-subtle" onClick={dismiss} disabled={busy}>{t.actions.cancel}</button>
        <button type="button" className="button button-primary" onClick={commit} disabled={busy || !selected.length}>{busyAction === "commit" ? ui.importCommitBusy : `${t.actions.confirmImport} (${selected.length})`}</button>
      </div>
    </section>}
  </section>;
}
