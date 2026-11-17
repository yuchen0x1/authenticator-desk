import { useEffect, useRef, useState, type FormEvent } from "react";
import { backupCommands, type AutoBackupStatus } from "../../lib/backup";
import { getMessages, type Locale } from "../../i18n";
import { backupMessages } from "../../i18n/backup";

export function AutoBackupSettings({locale,onError,notify}:{locale:Locale;onError:(error:unknown)=>void;notify:(message:string)=>void}) {
  const t=getMessages(locale), b=backupMessages(locale);
  const [status,setStatus]=useState<AutoBackupStatus|null>(null);
  const [interval,setInterval]=useState(24),[retention,setRetention]=useState(7);
  const [password,setPassword]=useState(""),[confirm,setConfirm]=useState("");
  const [busy,setBusy]=useState(false),[error,setError]=useState("");
  const live=useRef(true);
  useEffect(()=>{live.current=true;backupCommands.status().then(value=>{if(live.current){setStatus(value);setInterval(value.interval_hours);setRetention(value.retention)}}).catch(error=>{if(live.current)onError(error)});return()=>{live.current=false}},[onError]);
  async function configure(enabled:boolean) {
    if(enabled&&password!==confirm){setError(b.passwordMismatch);return}
    const secret=password;setPassword("");setConfirm("");setError("");setBusy(true);
    try { const value=await backupCommands.configure(enabled,enabled?secret:"",interval,retention);if(live.current&&value){setStatus(value);notify(t.status.saved)} }
    catch(error){if(live.current)onError(error)}finally{if(live.current)setBusy(false)}
  }
  function submit(event:FormEvent){event.preventDefault();void configure(true)}
  return <div className="settings-group"><div className="section-heading"><h3>{b.auto}</h3><span className="capability-tag">{status?.paused_locked?b.paused:status?.enabled?b.enabled:status?b.disabled:t.pages.loading}</span></div><p className="backup-hint">{b.autoHint}</p><form onSubmit={submit}><div className="setting-grid"><label>{b.interval}<input type="number" min={1} max={8760} required value={interval} onChange={event=>setInterval(Number(event.target.value))}/></label><label>{b.retention}<input type="number" min={1} max={100} required value={retention} onChange={event=>setRetention(Number(event.target.value))}/></label><label>{t.fields.backupPassword}<input type="password" autoComplete="new-password" required value={password} onChange={event=>setPassword(event.target.value)}/></label><label>{b.passwordConfirm}<input type="password" autoComplete="new-password" required value={confirm} onChange={event=>setConfirm(event.target.value)}/></label></div>{error&&<p role="alert">{error}</p>}<div className="backup-actions"><span className="text-sm text-[var(--muted)]">{b.lastBackup}: {status?.last_run?new Date(status.last_run*1000).toLocaleString(locale):"—"}</span><button type="button" className="button button-subtle" disabled={busy||!status?.enabled} onClick={()=>void configure(false)}>{b.disable}</button><button className="button button-primary" disabled={busy||!status}>{busy?t.pages.loading:b.configure}</button></div></form></div>;
}
