import { useCallback, useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getMessages, type Locale } from "./i18n";
import { errorMessage } from "./i18n/errors";
import { accountCommands, activityCommands, categoryCommands, settingsCommands, tauriInvoke, vaultCommands } from "./lib/tauri";
import { cacheLocale, cacheTheme, isSupportedLocale, readCachedLocale, readCachedTheme } from "./lib/uiPrefs";
import type { AccountInput, AccountView, AppSettings, AppSnapshot, CategoryView, CodeView } from "./lib/types";
import { AccountDialog } from "./components/accounts/AccountDialog";
import { AccountsPage } from "./pages/AccountsPage";
import { ImportPage } from "./pages/ImportPage";
import { SettingsPage } from "./pages/SettingsPage";
import { CategoriesPage } from "./pages/CategoriesPage";
import { DashboardPage } from "./pages/DashboardPage";
import { ActivityPage } from "./pages/ActivityPage";
import { BackupPage } from "./pages/BackupPage";
import { SecurityPage } from "./pages/SecurityPage";
import { AppShell, type Destination } from "./components/layout/AppShell";

export const defaultSettings: AppSettings = { locale:"zh-CN",theme:"dark",idle_timeout_secs:900,clipboard_ttl_secs:30,hide_codes:false,view_mode:"cards",animations:true,compact:false,font_size:14,default_algorithm:"SHA1",default_digits:6,default_period_secs:30,close_to_tray:false,minimize_to_tray:false,startup:false,backup_enabled:false,backup_interval_hours:24,backup_retention:10,show_next_code:false };
export type WorkspaceProps = { locale: Locale; accounts: AccountView[]; categories: CategoryView[]; codes: CodeView[]; settings: AppSettings; refresh: () => Promise<void>; notify: (message:string) => void; onError: (error:unknown) => void; onAdd: () => void; onEdit: (account:AccountView) => void; navigate: (page:Destination) => void; saveSettings: (value:AppSettings) => Promise<void> };

type WorkspaceState = "loading" | "ready" | "recovery" | "error" | "unavailable";

export default function App() {
  const [snapshot,setSnapshot] = useState<AppSnapshot|null>(null);
  const [workspace,setWorkspace] = useState<WorkspaceState>("loading");
  const [locale,setLocale] = useState<Locale>(readCachedLocale);
  const [settings,setSettings] = useState<AppSettings>(()=>({ ...defaultSettings, locale: readCachedLocale(), theme: readCachedTheme() }));
  const [accounts,setAccounts] = useState<AccountView[]>([]);
  const [categories,setCategories] = useState<CategoryView[]>([]);
  const [codes,setCodes] = useState<CodeView[]>([]);
  const [page,setPage] = useState<Destination>("dashboard");
  const [toast,setToast] = useState("");
  const [error,setError] = useState("");
  const [dialog,setDialog] = useState<AccountView|"new"|null>(null);
  const generation=useRef(0);
  const dialogGeneration=useRef(0);
  const codeGeneration=useRef(0);
  const hydrationRequest=useRef(0);
  const settingsRequest=useRef(0);
  const refreshRequest=useRef(0);
  const active=useRef(true);
  const snapshotRef=useRef<AppSnapshot|null>(null);
  const accountsRef=useRef<AccountView[]>([]);
  const settingsRef=useRef(defaultSettings);
  const localeRef=useRef<Locale>("zh-CN");
  const ready=useRef(false);
  const codesBlocked=useRef(true);
  const lastTouch=useRef(0);
  const restoreFocus=useRef<HTMLElement|null>(null);
  const t=getMessages(locale);
  localeRef.current=locale;

  const blockCodes=useCallback(()=>{ codesBlocked.current=true; codeGeneration.current++; setCodes([]); },[]);
  const applySettings=useCallback((value:AppSettings)=>{
    settingsRef.current=value;
    codesBlocked.current=value.hide_codes;
    setSettings(value);
    setLocale(isSupportedLocale(value.locale) ? value.locale : "zh-CN");
    // Remember only non-sensitive authenticated preferences for the next locked launch.
    cacheTheme(value.theme);
    cacheLocale(value.locale);
  },[]);
  const clearSensitive=useCallback(()=>{
    generation.current++;
    hydrationRequest.current++;
    settingsRequest.current++;
    ready.current=false;
    blockCodes();
    accountsRef.current=[];
    dialogGeneration.current++;
    setAccounts([]); setCategories([]); setDialog(null); setToast("");
    if(snapshotRef.current){
      snapshotRef.current={...snapshotRef.current,locked:true};
      setSnapshot(snapshotRef.current);
      setWorkspace("ready");
    }else setWorkspace("error");
  },[blockCodes]);
  const fail=useCallback((reason:unknown)=>{
    if((reason as {code?:string})?.code==="vault_locked")clearSensitive();
    setError(errorMessage(localeRef.current,reason));
  },[clearSensitive]);
  const hydrate=useCallback(async()=>{
    const token=generation.current;
    const request=++hydrationRequest.current;
    refreshRequest.current++;
    settingsRequest.current++;
    const current=()=>active.current&&token===generation.current&&request===hydrationRequest.current;
    ready.current=false;
    blockCodes();
    setWorkspace("loading"); setError("");
    let receivedSnapshot=false;
    let receivedSettings=false;
    try{
      const value=await tauriInvoke<AppSnapshot>("app_snapshot");
      if(!current())return;
      receivedSnapshot=true;
      snapshotRef.current=value; setSnapshot(value);
      if(value.locked){clearSensitive();return;}
      const valueSettings=await settingsCommands.get();
      if(!current())return;
      applySettings(valueSettings);
      receivedSettings=true;
      // Promise.all only reports its first rejection. A later locked response
      // from either metadata request must still revoke the recovery session.
      const metadataFailure=(reason:unknown)=>{
        if(current()&&(reason as {code?:string})?.code==="vault_locked")fail(reason);
        throw reason;
      };
      const [items,groups]=await Promise.all([accountCommands.list({search:"",favorite_only:false,sort:"manual"}).catch(metadataFailure),categoryCommands.list().catch(metadataFailure)]);
      if(!current())return;
      accountsRef.current=items; setAccounts(items); setCategories(groups);
      ready.current=true; setWorkspace("ready");
    }catch(reason){
      if(!current())return;
      blockCodes();
      if((reason as {code?:string})?.code==="vault_locked"){fail(reason);return;}
      // Only an unlocked snapshot plus authenticated settings may expose repair
      // tools. Failed metadata is never an empty, ready vault or an OTP source.
      accountsRef.current=[];
      dialogGeneration.current++;
      setAccounts([]); setCategories([]); setDialog(null);
      setWorkspace(receivedSettings?"recovery":!receivedSnapshot&&(reason as {code?:string})?.code==="backend_unavailable"?"unavailable":"error");
      setError(errorMessage(localeRef.current,reason));
    }
  },[applySettings,blockCodes,clearSensitive,fail]);
  const refresh=useCallback(async()=>{
    if(snapshotRef.current?.locked)return;
    if(workspace==="recovery"){await hydrate();return;}
    if(!ready.current)return;
    // Responses can arrive out of order; only the newest refresh may publish, so a
    // slow earlier request cannot restore accounts a later one already replaced.
    const request=++refreshRequest.current;
    const token=generation.current;
    const current=()=>active.current&&ready.current&&request===refreshRequest.current&&generation.current===token&&!snapshotRef.current?.locked;
    try{
      const [items,groups]=await Promise.all([accountCommands.list({search:"",favorite_only:false,sort:"manual"}),categoryCommands.list()]);
      if(current()){accountsRef.current=items;setAccounts(items);setCategories(groups);}
    }catch(reason){if(current())throw reason;}
  },[hydrate,workspace]);
  const syncSettings=useCallback(()=>{
    // The event is a privacy boundary, not just a request to repaint masked text.
    blockCodes();
    const request=++settingsRequest.current;
    const token=generation.current;
    if(snapshotRef.current?.locked)return;
    if(!ready.current){void hydrate();return;}
    void settingsCommands.get().then((value)=>{
      if(active.current&&token===generation.current&&request===settingsRequest.current&&!snapshotRef.current?.locked)applySettings(value);
    }).catch((reason)=>{
      if(!active.current||token!==generation.current||request!==settingsRequest.current)return;
      fail(reason);
      if((reason as {code?:string})?.code!=="vault_locked"){ready.current=false;dialogGeneration.current++;setDialog(null);setWorkspace("error");}
    });
  },[applySettings,blockCodes,fail,hydrate]);
  const notify=useCallback((message:string)=>setToast(message),[]);
  useLayoutEffect(()=>{document.documentElement.lang=locale;},[locale]);
  // The theme attribute lives on <html> as well: body background, native
  // select popups, scrollbars and color-scheme all resolve from the root.
  useLayoutEffect(()=>{document.documentElement.dataset.theme=settings.theme;},[settings.theme]);
  useLayoutEffect(()=>{document.documentElement.dataset.motion=settings.animations?"on":"off";},[settings.animations]);
  useEffect(()=>{if(!toast)return;const handle=window.setTimeout(()=>setToast(""),2500);return()=>window.clearTimeout(handle);},[toast]);
  useEffect(()=>{
    active.current=true; void hydrate();
    return()=>{active.current=false;generation.current++;codeGeneration.current++;};
  },[hydrate]);
  useEffect(()=>{
    let live=true;
    const stops:(()=>void)[]=[];
    for(const [event,handler] of [["vault://locked",clearSensitive],["settings://changed",syncSettings]] as const){
      void listen(event,handler).then((stop)=>{if(live)stops.push(stop);else stop();}).catch(()=>{});
    }
    return()=>{live=false;stops.forEach((stop)=>stop());};
  },[clearSensitive,syncSettings]);
  useEffect(()=>{
    if(!snapshot||snapshot.locked||workspace!=="ready")return;
    let cancelled=false;
    let busy=false;
    const tick=async()=>{
      if(busy)return;
      busy=true;
      const token=generation.current;
      const codeToken=codeGeneration.current;
      const current=()=>!cancelled&&active.current&&token===generation.current&&codeToken===codeGeneration.current&&!snapshotRef.current?.locked;
      try{
        const state=await vaultCommands.status();
        if(!current())return;
        if(state.locked){clearSensitive();return;}
        if(codesBlocked.current||!ready.current||!accountsRef.current.length)return;
        const values=await accountCommands.codes(accountsRef.current.map((item)=>item.id));
        if(!current()||codesBlocked.current)return;
        const latest=await vaultCommands.status();
        if(!current()||codesBlocked.current)return;
        if(latest.locked){clearSensitive();return;}
        if(latest.generation!==state.generation||latest.revision!==state.revision){syncSettings();return;}
        setCodes(values);
      }catch(reason){if(current()){setCodes([]);fail(reason);}}
      finally{busy=false;}
    };
    void tick();
    const timer=window.setInterval(()=>void tick(),1000);
    const focus=()=>void tick();
    const activity=()=>{if(Date.now()-lastTouch.current>10000){lastTouch.current=Date.now();void activityCommands.touch().catch(fail);}};
    window.addEventListener("focus",focus); document.addEventListener("visibilitychange",focus);
    window.addEventListener("pointerdown",activity); window.addEventListener("keydown",activity);
    return()=>{cancelled=true;window.clearInterval(timer);window.removeEventListener("focus",focus);document.removeEventListener("visibilitychange",focus);window.removeEventListener("pointerdown",activity);window.removeEventListener("keydown",activity);};
  },[snapshot?.locked,workspace,settings.hide_codes,clearSensitive,fail,syncSettings]);
  async function lockedForm(password:string,create:boolean){
    setError("");
    const token=generation.current;
    if(create){
      await vaultCommands.create(password);
      if(!active.current||token!==generation.current)return;
      // Creation succeeded even if the following unlock needs to be retried.
      if(snapshotRef.current){snapshotRef.current={...snapshotRef.current,vault_exists:true};setSnapshot(snapshotRef.current);}
    }
    await vaultCommands.unlock(password);
    if(!active.current||token!==generation.current)return;
    generation.current++;
    if(snapshotRef.current){snapshotRef.current={...snapshotRef.current,locked:false,vault_exists:true};setSnapshot(snapshotRef.current);}
    await hydrate();
  }
  async function lock(){clearSensitive();try{await vaultCommands.lock();}catch(reason){fail(reason);}}
  function showDialog(item:AccountView|"new"){dialogGeneration.current++;restoreFocus.current=document.activeElement as HTMLElement;setDialog(item);}
  function closeDialog(){dialogGeneration.current++;setDialog(null);restoreFocus.current?.focus();}
  async function saveAccount(input:AccountInput){
    const target=dialog;
    const token=generation.current;
    const dialogToken=dialogGeneration.current;
    const currentSession=()=>active.current&&ready.current&&token===generation.current&&!snapshotRef.current?.locked;
    const currentDialog=()=>currentSession()&&dialogToken===dialogGeneration.current;
    if(!target||!currentDialog())return;
    try{
      if(target==="new")await accountCommands.create(input);
      else await accountCommands.update(target.id,input);
    }catch(reason){if(currentDialog())throw reason;return;}
    // A completed native mutation must never close a newer session's draft.
    if(!currentDialog())return;
    closeDialog();notify(getMessages(localeRef.current).status.saved);
    try{await refresh();}catch(reason){if(currentSession())fail(reason);}
  }
  async function saveSettings(value:AppSettings){
    const token=generation.current;
    const request=++settingsRequest.current;
    const previous=settingsRef.current;
    blockCodes();
    if(value.hide_codes){settingsRef.current={...previous,hide_codes:true};setSettings(settingsRef.current);}
    try{
      await settingsCommands.save(value);
      if(!active.current||token!==generation.current)return;
      if(request===settingsRequest.current)applySettings(value);
      notify(getMessages(value.locale).status.saved);
    }catch(reason){
      if(!active.current||token!==generation.current)return;
      if(request===settingsRequest.current)applySettings(previous);
      throw reason;
    }
  }
  const recoveryPage=page==="backup"||page==="security";
  function navigate(value:Destination){if(workspace==="recovery"&&value!=="backup"&&value!=="security")return;setPage(value);}
  const props:WorkspaceProps={locale,accounts,categories,codes,settings,refresh,notify,onError:fail,onAdd:()=>showDialog("new"),onEdit:showDialog,navigate,saveSettings};
  return <div data-theme={settings.theme} data-motion={settings.animations?"on":"off"} data-compact={settings.compact?"on":"off"} className="app-root"><div inert={dialog?true:undefined}><AppShell locale={locale} page={page} locked={snapshot?.locked??true} onNavigate={navigate} onLock={()=>void lock()}>
    {error&&workspace!=="unavailable"&&<div role="alert" className="error-banner"><span>{error}</span><button onClick={()=>setError("")} aria-label={t.actions.dismissError} title={t.actions.dismissError}>×</button></div>}
    {workspace==="unavailable"?<div className="vault-gate"><div className="vault-emblem">◇</div><h2>{t.shell.unavailableTitle}</h2><p>{t.shell.unavailable}</p></div>:snapshot?.locked?<VaultGate locale={locale} exists={snapshot.vault_exists} onSubmit={lockedForm} onError={fail}/>:workspace==="loading"?<div className="loading-state" role="status">{t.shell.checking}</div>:workspace==="error"||workspace==="recovery"?<><div className={workspace==="recovery"&&recoveryPage?"workspace-feedback":"vault-gate"}><div className="vault-emblem">◇</div><h2>{t.status.error}</h2><button className="button button-primary" onClick={()=>void hydrate()}>{t.desktop.retry}</button></div>{workspace==="recovery"&&(page==="backup"?<BackupPage {...props}/>:page==="security"?<SecurityPage {...props}/>:null)}</>:<>{page==="dashboard"?<DashboardPage {...props}/>:page==="codes"||page==="accounts"||page==="favorites"?<AccountsPage {...props} mode={page}/>:page==="categories"?<CategoriesPage {...props}/>:page==="import"?<ImportPage {...props}/>:page==="backup"?<BackupPage {...props}/>:page==="security"?<SecurityPage {...props}/>:page==="activity"?<ActivityPage {...props}/>:<SettingsPage {...props}/>}</>}
  </AppShell></div>{toast&&<div role="status" className="toast">✓ {toast}</div>}{dialog&&!snapshot?.locked&&workspace==="ready"&&<AccountDialog key={dialogGeneration.current} account={dialog==="new"?undefined:dialog} categories={categories} defaults={{algorithm:settings.default_algorithm,digits:settings.default_digits,period:settings.default_period_secs}} t={{...t.fields,...t.actions,...t.status,loading:t.pages.loading,retry:t.desktop.retry,required:t.status.invalid,uncategorized:t.fields.uncategorized,secretPrivacy:t.pages.accountsHint,secretKeep:t.fields.secretKeep}} onClose={closeDialog} onSave={saveAccount}/>}</div>;
}

function VaultGate({locale,exists,onSubmit,onError}:{locale:Locale;exists:boolean;onSubmit:(password:string,create:boolean)=>Promise<void>;onError:(reason:unknown)=>void}){const t=getMessages(locale);const [password,setPassword]=useState("");const [confirm,setConfirm]=useState("");const [pending,setPending]=useState(false);const [error,setError]=useState("");async function submit(event:FormEvent){event.preventDefault();if(!exists&&password.length<12){setError(t.status.weakPassword);return;}if(!exists&&password!==confirm){setError(t.status.mismatch);return;}const value=password;setPassword("");setConfirm("");setPending(true);setError("");try{await onSubmit(value,!exists);}catch(reason){onError(reason);}finally{setPending(false);}}return <div className="vault-gate"><div className="vault-emblem" aria-hidden="true">◇</div><h2>{exists?t.shell.lockedTitle:t.shell.createTitle}</h2><p>{exists?t.shell.lockedBody:t.shell.createBody}</p><form onSubmit={submit} className="mt-7 grid w-full max-w-sm gap-4"><label>{t.shell.password}<input autoFocus type="password" autoComplete={exists?"current-password":"new-password"} value={password} onChange={(event)=>setPassword(event.target.value)} required minLength={exists?undefined:12}/></label>{!exists&&<label>{t.shell.confirmPassword}<input type="password" autoComplete="new-password" value={confirm} onChange={(event)=>setConfirm(event.target.value)} required/></label>}{error&&<p role="alert" className="field-error">{error}</p>}<button className="button button-primary" disabled={pending} type="submit">{pending?t.pages.loading:exists?t.shell.unlock:t.shell.createVault}</button></form></div>;}