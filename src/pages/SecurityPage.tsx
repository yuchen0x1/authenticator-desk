import { useCallback, useEffect, useRef, useState } from "react";
import { backupCommands, type SecurityAudit } from "../lib/backup";
import { getMessages } from "../i18n";
import { backupMessages } from "../i18n/backup";
import type { WorkspaceProps } from "../App";

export function SecurityPage({locale,settings,onError,navigate}:WorkspaceProps){
  const t=getMessages(locale),b=backupMessages(locale);
  const [report,setReport]=useState<SecurityAudit|null>(null),[busy,setBusy]=useState(false);
  const live=useRef(true);
  const inspect=useCallback(async()=>{setBusy(true);try{const value=await backupCommands.audit();if(live.current)setReport(value)}catch(error){if(live.current)onError(error)}finally{if(live.current)setBusy(false)}},[onError]);
  useEffect(()=>{live.current=true;void inspect();return()=>{live.current=false}},[inspect]);
  const count=(value:number|undefined)=>value===undefined?t.status.notChecked:String(value);
  return <section><div className="page-heading"><div><h2>{t.pages.security}</h2><p>{b.auditHint}</p></div><button className="button button-subtle" disabled={busy} onClick={()=>void inspect()}>{busy?t.pages.loading:t.actions.refresh}</button></div><div className="security-list"><SecurityRow label={t.status.integrity} value={report?report.integrity.ok?t.status.ok:t.status.error:t.status.notChecked}/><SecurityRow label={b.corrupt} value={count(report?.corrupt_record_count)}/><SecurityRow label={b.duplicates} value={count(report?.duplicate_count)}/><SecurityRow label={b.invalid} value={count(report?.invalid_config_count)}/><SecurityRow label={b.plaintext} value={report?String(report.plaintext_file_findings.length):t.status.notChecked}/><SecurityRow label={b.permissions} value={b.permissionsUnknown}/><SecurityRow label={b.clock} value={b.clockUnknown}/><SecurityRow label={t.fields.idleTimeout} value={`${settings.idle_timeout_secs} s`}/><SecurityRow label={b.backupConfigured} value={report?report.backup_configured?b.yes:b.no:t.status.notChecked}/><SecurityRow label={b.backupCount} value={count(report?.backup_count)}/><SecurityRow label={b.lastBackup} value={report?.last_backup_at?new Date(Number(report.last_backup_at)*1000).toLocaleString(locale):b.none}/></div><p className="backup-hint mt-4">{b.plaintextHint}</p>{report?.plaintext_file_findings.length? <ul className="audit-findings">{report.plaintext_file_findings.map((file,index)=><li key={index}>{file==="scan_limit_reached"?b.scanLimit:file}</li>)}</ul>:null}<div className="backup-actions"><span className="text-sm text-[var(--muted)]">{report?`${b.checked}: ${new Date(report.checked_at).toLocaleString(locale)}`:t.status.notChecked}</span><button className="button button-subtle" onClick={()=>navigate("backup")}>{b.openBackup}</button><button className="button button-subtle" onClick={()=>navigate("settings")}>{t.settings.changePassword}</button></div></section>;
}
function SecurityRow({label,value}:{label:string;value:string}){return <div className="security-row"><span>{label}</span><strong>{value}</strong></div>}
