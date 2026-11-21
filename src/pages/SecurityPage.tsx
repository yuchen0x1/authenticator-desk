import { useCallback, useEffect, useRef, useState } from "react";
import { backupCommands, type SecurityAudit } from "../lib/backup";
import { getMessages } from "../i18n";
import { backupMessages } from "../i18n/backup";
import { workspaceMessages } from "../i18n/workspace";
import type { WorkspaceProps } from "../App";

export function SecurityPage({ locale, settings, onError, navigate }: WorkspaceProps) {
  const t = getMessages(locale), b = backupMessages(locale), w = workspaceMessages(locale);
  const [report, setReport] = useState<SecurityAudit | null>(null);
  const [busy, setBusy] = useState(false), [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const live = useRef(true);

  const inspect = useCallback(async () => {
    setBusy(true);
    setFailed(false);
    try {
      const value = await backupCommands.audit();
      if (live.current) setReport(value);
    } catch (error) {
      if (live.current) {
        if ((error as { code?: string })?.code === "vault_locked") onError(error);
        else setFailed(true);
      }
    } finally {
      if (live.current) setBusy(false);
    }
  }, [onError]);

  useEffect(() => {
    live.current = true;
    void inspect();
    return () => { live.current = false; };
  }, [inspect, attempt]);

  const unavailable = busy ? t.pages.loading : failed ? w.unavailable : t.status.notChecked;
  const count = (value: number | undefined) => value === undefined ? unavailable : String(value);
  const checkedAt = report ? new Date(report.checked_at).toLocaleString(locale) : unavailable;

  return <section aria-busy={busy}>
    <div className="page-heading"><div><h2>{t.pages.security}</h2><p>{b.auditHint}</p></div>
      <button className="button button-subtle" disabled={busy} onClick={() => setAttempt(value => value + 1)}>{busy ? t.pages.loading : failed ? w.retry : t.actions.refresh}</button>
    </div>
    {failed && <div className="workspace-feedback"><p role="alert">{report ? w.auditRefreshError : w.auditLoadError}</p>
      {report && <span className="workspace-wrap">{w.lastSuccessfulCheck}: {checkedAt}</span>}
    </div>}
    <div className="security-list">
      <SecurityRow label={t.status.integrity} value={report ? report.integrity.ok ? t.status.ok : t.status.error : unavailable} />
      <SecurityRow label={b.corrupt} value={count(report?.corrupt_record_count)} />
      <SecurityRow label={b.duplicates} value={count(report?.duplicate_count)} />
      <SecurityRow label={b.invalid} value={count(report?.invalid_config_count)} />
      <SecurityRow label={b.plaintext} value={report ? String(report.plaintext_file_findings.length) : unavailable} />
      <SecurityRow label={b.permissions} value={report ? b.permissionsUnknown : unavailable} />
      <SecurityRow label={b.clock} value={report ? b.clockUnknown : unavailable} />
      <SecurityRow label={t.fields.idleTimeout} value={`${settings.idle_timeout_secs} s`} />
      <SecurityRow label={b.backupConfigured} value={report ? report.backup_configured ? b.yes : b.no : unavailable} />
      <SecurityRow label={b.backupCount} value={count(report?.backup_count)} />
      <SecurityRow label={b.lastBackup} value={report ? report.last_backup_at ? new Date(Number(report.last_backup_at) * 1000).toLocaleString(locale) : b.none : unavailable} />
    </div>
    <p className="backup-hint mt-4 workspace-wrap">{b.plaintextHint}</p>
    {report?.plaintext_file_findings.length ? <ul className="audit-findings">{report.plaintext_file_findings.map((file, index) => <li className="workspace-wrap" key={index}>{file === "scan_limit_reached" ? b.scanLimit : file}</li>)}</ul> : null}
    <div className="backup-actions"><span className="text-sm text-[var(--muted)] workspace-wrap">{report ? `${b.checked}: ${checkedAt}` : `${b.checked}: ${unavailable}`}</span>
      <button className="button button-subtle" onClick={() => navigate("backup")}>{b.openBackup}</button>
      <button className="button button-subtle" onClick={() => navigate("settings")}>{t.settings.changePassword}</button>
    </div>
  </section>;
}

function SecurityRow({ label, value }: { label: string; value: string }) {
  return <div className="security-row"><span className="workspace-wrap">{label}</span><strong className="workspace-wrap">{value}</strong></div>;
}
