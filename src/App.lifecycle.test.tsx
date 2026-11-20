import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App, { defaultSettings } from "./App";
import type { AccountView, AppSettings, AppSnapshot, CodeView } from "./lib/types";

const ipc = vi.hoisted(() => ({ invoke: vi.fn(), handlers: new Map<string, () => void>() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: ipc.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (event: string, handler: () => void) => {
  ipc.handlers.set(event, handler);
  return () => { if (ipc.handlers.get(event) === handler) ipc.handlers.delete(event); };
}) }));
const account: AccountView = { id:"account-one",issuer:"GitHub",account_name:"alice",algorithm:"SHA1",digits:6,kind:"totp",period_secs:30,t0:"0",hotp_counter:"0",category_id:null,tags:[],notes_present:false,favorite:false,sort_order:0,created_at:"2026-11-17T00:00:00Z",updated_at:"2026-11-17T00:00:00Z" };
const freshCode = (value = "123456"): CodeView => ({account_id:account.id,code:value,remaining_secs:30,progress:1,generated_at:Math.floor(Date.now()/1000),clock_offset_secs:0,wall_clock_jump:false});
function deferred<T>() { let resolve!: (value:T) => void; let reject!: (reason:unknown) => void; const promise = new Promise<T>((yes,no)=>{resolve=yes;reject=no;}); return {promise,resolve,reject}; }
let snapshot: AppSnapshot;
let settings: AppSettings;
function reply(command:string, payload?:{settings?:AppSettings}) {
  if(command === "app_snapshot") return {...snapshot};
  if(command === "settings") return {...settings};
  if(command === "save_settings") {settings = payload!.settings!;ipc.handlers.get("settings://changed")?.();return;}
  if(command === "create_vault") {snapshot.vault_exists=true;return;}
  if(command === "unlock_vault") {snapshot.locked=false;return {exists:true,locked:false,generation:1,revision:1};}
  if(command === "list_accounts") return [account];
  if(command === "list_categories") return [];
  if(command === "vault_status") return {exists:true,locked:snapshot.locked,generation:1,revision:1};
  if(command === "generate_codes") return [freshCode()];
  if(command === "dashboard") return {account_count:1,category_count:0,favorite_count:0,copies_today:0,recent_accounts:[account]};
  if(command === "desktop_config") return {close_to_tray:false,minimize_to_tray:false,startup:false,notifications:false,show_shortcut:"",quick_panel_shortcut:"",lock_shortcut:""};
  if(command === "desktop_capabilities") return {tray:false,shortcuts:false,startup:false,notifications:false,clipboard:true,qr_image:true,camera:false,camera_reason:"unsupported",warnings:[]};
  if(command === "backup_auto_status") return {enabled:false,paused_locked:false,interval_hours:24,retention:10,last_run:null};
  if(command === "security_audit") return {checked_at:"2026-11-17T00:00:00Z",locked:false,integrity:{ok:true,user_version:1},corrupt_record_count:1,duplicate_count:0,invalid_config_count:0,plaintext_file_findings:[],permission_findings:"unknown",backup_count:1,last_backup_at:null,backup_configured:false,time_status:"unknown"};
  if(command === "update_version") return {version:"0.2.1",available:false};
  return undefined;
}
const codeCalls = () => ipc.invoke.mock.calls.filter(([command])=>command === "generate_codes");
async function showCodes() { await screen.findByRole("button", {name:/Add account/});fireEvent.click(screen.getByRole("button",{name:"Codes"}));await screen.findByText("alice"); }
beforeEach(() => {
  snapshot={locked:false,vault_exists:true,locale:"en",theme:"dark"};
  settings={...defaultSettings,locale:"en"};
  ipc.handlers.clear();
  ipc.invoke.mockReset().mockImplementation(async (command:string,payload?:{settings?:AppSettings})=>reply(command,payload));
});

describe("vault UI boundary", () => {
  it("removes accounts immediately and rejects late code results after a lock event", async () => {
    const pending=deferred<CodeView[]>();
    ipc.invoke.mockImplementation(async (command:string)=>command === "generate_codes" ? pending.promise : reply(command));
    render(<App />);
    await showCodes();
    await waitFor(()=>expect(codeCalls()).toHaveLength(1));
    act(()=>ipc.handlers.get("vault://locked")?.());
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    await act(async()=>pending.resolve([freshCode()]));
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
  });

  it("does not request OTPs before settings and account hydration finish", async () => {
    const pendingSettings=deferred<AppSettings>();
    const pendingAccounts=deferred<AccountView[]>();
    ipc.invoke.mockImplementation(async (command:string)=>command === "settings" ? pendingSettings.promise : command === "list_accounts" ? pendingAccounts.promise : reply(command));
    render(<App />);
    expect(await screen.findByText("Reading vault")).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Codes"}));
    expect(screen.queryByText("No accounts here yet")).not.toBeInTheDocument();
    expect(codeCalls()).toHaveLength(0);
    await act(async()=>pendingSettings.resolve(settings));
    expect(screen.getByText("Reading vault")).toBeVisible();
    expect(codeCalls()).toHaveLength(0);
    await act(async()=>pendingAccounts.resolve([account]));
    expect(await screen.findByText("123 456")).toBeVisible();
  });

  it.each(["success", "failure"])("ignores a superseded account refresh %s", async (outcome) => {
    const older = deferred<AccountView[]>();
    const newer = deferred<AccountView[]>();
    render(<App />);
    await showCodes();
    let lists = 0;
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === "list_accounts") return ++lists === 1 ? older.promise : newer.promise;
      return reply(command);
    });
    fireEvent.click(screen.getByRole("button", {name:"Add to favorites: GitHub · alice"}));
    await waitFor(()=>expect(lists).toBe(1));
    fireEvent.click(screen.getByRole("button", {name:"Refresh"}));
    await waitFor(()=>expect(lists).toBe(2));
    await act(async()=>newer.resolve([{...account,account_name:"newest"}]));
    expect(screen.getByText("newest")).toBeVisible();
    await act(async()=>{
      if (outcome === "success") older.resolve([account]);
      else older.reject({code:"storage"});
    });
    expect(screen.getByText("newest")).toBeVisible();
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("honors saved hide_codes without generating or copying OTPs", async () => {
    settings.hide_codes=true;
    render(<App />);
    await showCodes();
    expect(screen.getByText("••••••")).toBeVisible();
    expect(screen.getByRole("button",{name:"Copy code: GitHub · alice"})).toBeDisabled();
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(codeCalls()).toHaveLength(0);
    fireEvent.click(screen.getByRole("button",{name:"Copy code: GitHub · alice"}));
    expect(ipc.invoke).not.toHaveBeenCalledWith("copy_code",expect.anything());
  });

  it("revokes visible and delayed codes synchronously on settings changes", async () => {
    render(<App />);
    await showCodes();
    expect(await screen.findByText("123 456")).toBeVisible();
    const pendingCodes=deferred<CodeView[]>();
    const pendingSettings=deferred<AppSettings>();
    ipc.invoke.mockImplementation(async (command:string)=>command === "generate_codes" ? pendingCodes.promise : command === "settings" ? pendingSettings.promise : reply(command));
    act(()=>window.dispatchEvent(new Event("focus")));
    await waitFor(()=>expect(codeCalls()).toHaveLength(2));
    act(()=>ipc.handlers.get("settings://changed")?.());
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Copy code: GitHub · alice"})).toBeDisabled();
    await act(async()=>pendingCodes.resolve([freshCode("999888")]));
    expect(screen.queryByText("999 888")).not.toBeInTheDocument();
    settings={...settings,hide_codes:true,locale:"ja"};
    await act(async()=>pendingSettings.resolve(settings));
    expect(screen.getByText("••••••")).toBeVisible();
    expect(document.documentElement.lang).toBe("ja");
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(codeCalls()).toHaveLength(2);
  });

  it.each(["Add account", "Edit account: GitHub · alice"])("keeps retry and lock interactive after settings failure closes %s", async (action) => {
    render(<App />);
    await showCodes();
    fireEvent.click(screen.getByRole("button", {name:action === "Add account" ? /Add account/ : action}));
    expect(await screen.findByRole("dialog")).toBeVisible();
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === "settings")throw {code:"storage"};
      return reply(command);
    });
    act(()=>ipc.handlers.get("settings://changed")?.());
    await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const retry=screen.getByRole("button", {name:"Retry"});
    expect(retry.closest("[inert]")).toBeNull();
    expect(screen.getByRole("button", {name:"Lock now"}).closest("[inert]")).toBeNull();
    ipc.invoke.mockImplementation(async (command:string)=>reply(command));
    fireEvent.click(retry);
    expect(await screen.findByText("alice")).toBeVisible();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("clears local codes immediately on hide and obtains fresh codes only after reveal", async () => {
    render(<App />);
    await showCodes();
    await screen.findByText("123 456");
    fireEvent.click(screen.getByRole("button",{name:"Hide codes by default"}));
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    await waitFor(()=>expect(screen.getByRole("button",{name:"Hide codes by default"})).toHaveAttribute("aria-pressed","true"));
    const calls=codeCalls().length;
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(codeCalls()).toHaveLength(calls);
    fireEvent.click(screen.getByRole("button",{name:"Hide codes by default"}));
    await waitFor(()=>expect(screen.getByRole("button",{name:"Hide codes by default"})).toHaveAttribute("aria-pressed","false"));
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(await screen.findByText("123 456")).toBeVisible();
    expect(codeCalls().length).toBeGreaterThan(calls);
  });

  it("clears even an unexpired cached code when refresh fails and disables Copy", async () => {
    render(<App />);
    await showCodes();
    await screen.findByText("123 456");
    ipc.invoke.mockImplementation(async (command:string)=>{if(command === "generate_codes")throw {code:"storage"};return reply(command);});
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(await screen.findByRole("alert")).toHaveTextContent("The local vault could not be read.");
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Copy code: GitHub · alice"})).toBeDisabled();
  });

  it.each(["settings","list_accounts","list_categories"])("offers recovery for %s hydration errors instead of an unavailable or empty vault", async (failedCommand) => {
    let failed=false;
    ipc.invoke.mockImplementation(async (command:string)=>{if(command === failedCommand&&!failed){failed=true;throw {code:"storage"};}return reply(command);});
    render(<App />);
    expect(await screen.findByRole("button",{name:"Retry"})).toBeVisible();
    expect(screen.queryByText("Desktop backend unavailable")).not.toBeInTheDocument();
    expect(screen.queryByText("No accounts here yet")).not.toBeInTheDocument();
    expect(screen.queryByRole("button",{name:"Unlock vault"})).not.toBeInTheDocument();
    expect(codeCalls()).toHaveLength(0);
    fireEvent.click(screen.getByRole("button",{name:"Retry"}));
    await showCodes();
    expect(await screen.findByText("123 456")).toBeVisible();
  });

  it.each([
    ["list_accounts", "authentication_failed", "The password or authenticated data could not be verified."],
    ["list_accounts", "storage", "The local vault could not be read."],
    ["list_categories", "crypto", "The encrypted operation failed."],
  ])("restricts %s %s failures after unlock to recovery tools until a fresh retry succeeds", async (failedCommand, code, message) => {
    snapshot.locked=true;
    let failing=true;
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === failedCommand&&failing)throw {code};
      return reply(command);
    });
    render(<App />);
    await screen.findByRole("button",{name:"Unlock vault"});
    fireEvent.change(screen.getByLabelText("Master password"),{target:{value:"long-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Unlock vault"}));
    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(screen.getByRole("button",{name:"Retry"})).toBeVisible();
    expect(screen.queryByText("No accounts here yet")).not.toBeInTheDocument();
    expect(screen.queryByRole("button",{name:"Unlock vault"})).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button",{name:"Backup"}));
    expect(await screen.findByRole("heading",{name:"Encrypted backup"})).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent(message);
    expect(screen.getByRole("button",{name:"Retry"})).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Security"}));
    expect(await screen.findByRole("heading",{name:"Security center"})).toBeVisible();
    await waitFor(()=>expect(ipc.invoke).toHaveBeenCalledWith("security_audit",undefined));
    for(const name of ["Overview","Codes","Accounts","Favorites","Categories","Import","Activity","Settings","Change master password"]){
      fireEvent.click(screen.getByRole("button",{name}));
      expect(screen.getByRole("heading",{name:"Security center"})).toBeVisible();
    }
    expect(screen.getByRole("alert")).toHaveTextContent(message);
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(codeCalls()).toHaveLength(0);
    expect(ipc.invoke).not.toHaveBeenCalledWith("dashboard",undefined);

    failing=false;
    fireEvent.click(screen.getByRole("button",{name:"Retry"}));
    await waitFor(()=>expect(screen.queryByRole("button",{name:"Retry"})).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole("button",{name:"Codes"}));
    expect(await screen.findByText("alice")).toBeVisible();
    expect(await screen.findByText("123 456")).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Import"}));
    expect(screen.getByRole("heading",{name:"Import accounts"})).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(ipc.invoke.mock.calls.filter(([command])=>command === "unlock_vault")).toHaveLength(1);
  });

  it.each(["app_snapshot","settings"])("keeps %s failures fail-closed rather than exposing recovery tools", async (failedCommand) => {
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === failedCommand)throw {code:"authentication_failed"};
      return reply(command);
    });
    render(<App />);
    await screen.findByRole("alert");
    const chinese=failedCommand === "app_snapshot";
    for(const name of chinese?["备份","安全"]:["Backup","Security"]){
      fireEvent.click(screen.getByRole("button",{name}));
    }
    expect(screen.getByRole("button",{name:chinese?"重试":"Retry"})).toBeVisible();
    expect(screen.queryByRole("heading",{name:/Encrypted backup|Security center|加密备份|安全中心/})).not.toBeInTheDocument();
    expect(ipc.invoke).not.toHaveBeenCalledWith("backup_auto_status",undefined);
    expect(ipc.invoke).not.toHaveBeenCalledWith("security_audit",undefined);
    expect(codeCalls()).toHaveLength(0);
  });

  it.each(["settings","list_accounts","list_categories"])("keeps a vault_locked %s failure behind the unlock gate", async (failedCommand) => {
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === failedCommand)throw {code:"vault_locked"};
      return reply(command);
    });
    render(<App />);
    expect(await screen.findByRole("button",{name:"Unlock vault"})).toBeVisible();
    for(const name of ["Backup","Security"]){fireEvent.click(screen.getByRole("button",{name}));}
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    expect(screen.queryByRole("button",{name:"Retry"})).not.toBeInTheDocument();
    expect(ipc.invoke).not.toHaveBeenCalledWith("backup_auto_status",undefined);
    expect(ipc.invoke).not.toHaveBeenCalledWith("security_audit",undefined);
    expect(codeCalls()).toHaveLength(0);
  });

  it.each(["list_accounts","list_categories"])("honors a delayed %s vault_locked after the other metadata request failed", async (lockedCommand) => {
    const pending=deferred<never>();
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === lockedCommand)return pending.promise;
      if(command === "list_accounts"||command === "list_categories")throw {code:"authentication_failed"};
      return reply(command);
    });
    render(<App />);
    await screen.findByRole("button",{name:"Retry"});
    fireEvent.click(screen.getByRole("button",{name:"Backup"}));
    expect(screen.getByRole("heading",{name:"Encrypted backup"})).toBeVisible();
    await act(async()=>pending.reject({code:"vault_locked"}));
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    expect(screen.queryByRole("heading",{name:"Encrypted backup"})).not.toBeInTheDocument();
    expect(screen.queryByRole("button",{name:"Retry"})).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("The vault is locked. Unlock it to continue.");
    expect(codeCalls()).toHaveLength(0);
  });

  it.each(["success","failure"])("ignores a recovery retry's late %s after a lock", async (outcome) => {
    const pending=deferred<AccountView[]>();
    let lists=0;
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === "list_accounts"){
        if(++lists === 1)throw {code:"authentication_failed"};
        return pending.promise;
      }
      return reply(command);
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button",{name:"Retry"}));
    await waitFor(()=>expect(lists).toBe(2));
    act(()=>{snapshot.locked=true;ipc.handlers.get("vault://locked")?.();});
    await act(async()=>{
      if(outcome === "success")pending.resolve([account]);
      else pending.reject({code:"storage"});
    });
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Backup"}));
    expect(screen.queryByRole("heading",{name:"Encrypted backup"})).not.toBeInTheDocument();
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(codeCalls()).toHaveLength(0);
  });

  it.each(["success","failure"])("ignores a superseded recovery hydration's %s", async (outcome) => {
    const older=deferred<AccountView[]>();
    let lists=0;
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === "list_accounts"){
        if(++lists === 2)return older.promise;
        throw {code:"storage"};
      }
      return reply(command);
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button",{name:"Retry"}));
    await waitFor(()=>expect(lists).toBe(2));
    act(()=>ipc.handlers.get("settings://changed")?.());
    await screen.findByRole("button",{name:"Retry"});
    expect(lists).toBe(3);
    await act(async()=>{
      if(outcome === "success")older.resolve([account]);
      else older.reject({code:"authentication_failed"});
    });
    fireEvent.click(screen.getByRole("button",{name:"Codes"}));
    expect(screen.getByRole("button",{name:"Retry"})).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("The local vault could not be read.");
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(codeCalls()).toHaveLength(0);
    fireEvent.click(screen.getByRole("button",{name:"Backup"}));
    expect(screen.getByRole("heading",{name:"Encrypted backup"})).toBeVisible();
  });

  it("does not republish stale accounts or codes from before failed rehydration", async () => {
    render(<App />);
    await showCodes();
    await screen.findByText("123 456");
    const olderAccounts=deferred<AccountView[]>();
    const olderCodes=deferred<CodeView[]>();
    let lists=0;
    let failSettings=true;
    ipc.invoke.mockImplementation(async (command:string)=>{
      if(command === "generate_codes")return olderCodes.promise;
      if(command === "settings"&&failSettings)throw {code:"storage"};
      if(command === "list_accounts"){
        if(++lists === 1)return olderAccounts.promise;
        throw {code:"authentication_failed"};
      }
      return reply(command);
    });
    fireEvent.click(screen.getByRole("button",{name:"Refresh"}));
    await waitFor(()=>expect(lists).toBe(1));
    act(()=>window.dispatchEvent(new Event("focus")));
    await waitFor(()=>expect(codeCalls()).toHaveLength(2));
    act(()=>ipc.handlers.get("settings://changed")?.());
    await screen.findByRole("button",{name:"Retry"});
    failSettings=false;
    fireEvent.click(screen.getByRole("button",{name:"Retry"}));
    await screen.findByRole("button",{name:"Retry"});
    expect(lists).toBe(2);
    await act(async()=>{olderAccounts.resolve([account]);olderCodes.resolve([freshCode("999888")]);});
    fireEvent.click(screen.getByRole("button",{name:"Codes"}));
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(screen.queryByText("999 888")).not.toBeInTheDocument();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Retry"})).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("The password or authenticated data could not be verified.");
    act(()=>window.dispatchEvent(new Event("focus")));
    expect(codeCalls()).toHaveLength(2);
  });

  it("allows retrying a recoverable snapshot error without inventing vault existence", async () => {
    ipc.invoke.mockRejectedValueOnce({code:"storage"});
    render(<App />);
    expect(await screen.findByRole("button",{name:"重试"})).toBeVisible();
    expect(screen.queryByText("桌面安全后端不可用")).not.toBeInTheDocument();
    expect(screen.queryByRole("button",{name:"创建保险库"})).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"重试"}));
    await showCodes();
  });

  it("keeps a successfully created vault when unlock fails and retries unlock, not create", async () => {
    snapshot={...snapshot,locked:true,vault_exists:false};
    let unlocks=0;
    ipc.invoke.mockImplementation(async (command:string)=>{if(command === "unlock_vault"&&unlocks++===0)throw {code:"authentication_failed"};return reply(command);});
    render(<App />);
    await screen.findByRole("button",{name:"Create vault"});
    fireEvent.change(screen.getByLabelText("Master password"),{target:{value:"long-password"}});
    fireEvent.change(screen.getByLabelText("Confirm password"),{target:{value:"long-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Create vault"}));
    expect(await screen.findByRole("alert")).toHaveTextContent("The password or authenticated data could not be verified.");
    expect(screen.queryByLabelText("Confirm password")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Master password"),{target:{value:"long-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Unlock vault"}));
    await showCodes();
    expect(ipc.invoke.mock.calls.filter(([command])=>command === "create_vault")).toHaveLength(1);
  });

  it("recovers hydration after unlock without asking for the password again", async () => {
    snapshot.locked=true;
    let failed=false;
    ipc.invoke.mockImplementation(async (command:string)=>{if(command === "list_accounts"&&!failed){failed=true;throw {code:"storage"};}return reply(command);});
    render(<App />);
    await screen.findByRole("button",{name:"Unlock vault"});
    fireEvent.change(screen.getByLabelText("Master password"),{target:{value:"long-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Unlock vault"}));
    expect(await screen.findByRole("button",{name:"Retry"})).toBeVisible();
    expect(screen.queryByRole("button",{name:"Unlock vault"})).not.toBeInTheDocument();
    expect(screen.queryByText("No accounts here yet")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Retry"}));
    await showCodes();
    expect(ipc.invoke.mock.calls.filter(([command])=>command === "unlock_vault")).toHaveLength(1);
  });

  it("ignores delayed settings and account hydration after a lock", async () => {
    const pending=deferred<AccountView[]>();
    ipc.invoke.mockImplementation(async (command:string)=>command === "list_accounts" ? pending.promise : reply(command));
    render(<App />);
    await waitFor(()=>expect(ipc.invoke).toHaveBeenCalledWith("list_accounts",expect.anything()));
    act(()=>ipc.handlers.get("vault://locked")?.());
    await act(async()=>pending.resolve([account]));
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(codeCalls()).toHaveLength(0);
  });

  it("publishes the saved theme at the document root only after Save", async () => {
    settings.theme="light";
    render(<App />);
    await screen.findByRole("button",{name:/Add account/});
    expect(document.documentElement).toHaveAttribute("data-theme","light");
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    fireEvent.change(screen.getByLabelText("Appearance"),{target:{value:"dark"}});
    expect(document.documentElement).toHaveAttribute("data-theme","light");
    fireEvent.click(screen.getByRole("button",{name:"Save"}));
    await waitFor(()=>expect(document.documentElement).toHaveAttribute("data-theme","dark"));
    expect(ipc.invoke).toHaveBeenCalledWith("save_settings",{settings:expect.objectContaining({theme:"dark"})});
    act(()=>ipc.handlers.get("vault://locked")?.());
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    expect(document.documentElement).toHaveAttribute("data-theme","dark");
  });

  it("starts in the remembered appearance while the vault is still locked", async () => {
    snapshot={locked:true,vault_exists:true,locale:"en",theme:"dark"};
    settings={...defaultSettings,locale:"en",theme:"light"};
    localStorage.setItem("authdesk.theme","light");
    render(<App />);
    // The lock screen renders before any settings call can answer, so the cached
    // choice is the only thing keeping it consistent with the last session.
    expect(await screen.findByRole("button",{name:"Unlock vault"})).toBeVisible();
    expect(document.documentElement).toHaveAttribute("data-theme","light");
    expect(document.querySelector(".app-root")).toHaveAttribute("data-theme","light");
    expect(ipc.invoke).not.toHaveBeenCalledWith("settings");
    localStorage.clear();
  });

  it("remembers a newly saved appearance for the next launch", async () => {
    render(<App />);
    await screen.findByRole("button",{name:/Add account/});
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    fireEvent.change(screen.getByLabelText("Appearance"),{target:{value:"light"}});
    fireEvent.click(screen.getByRole("button",{name:"Save"}));
    await waitFor(()=>expect(localStorage.getItem("authdesk.theme")).toBe("light"));
    localStorage.clear();
  });

  it("keeps the saved theme after a rejected settings save", async () => {
    render(<App />);
    await screen.findByRole("button",{name:/Add account/});
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    fireEvent.change(screen.getByLabelText("Appearance"),{target:{value:"light"}});
    ipc.invoke.mockImplementation(async (command:string,payload?:{settings?:AppSettings})=>{
      if(command === "save_settings")throw {code:"storage"};
      return reply(command,payload);
    });
    fireEvent.click(screen.getByRole("button",{name:"Save"}));
    await screen.findByRole("alert");
    expect(document.documentElement).toHaveAttribute("data-theme","dark");
    expect(screen.getByRole("button",{name:"Save"})).toBeEnabled();
  });

  it("changes the runtime and document language only after saving real settings", async () => {
    render(<App />);
    await screen.findByRole("button",{name:/Add account/});
    expect(document.documentElement.lang).toBe("en");
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    fireEvent.change(screen.getByLabelText("Language"),{target:{value:"ja"}});
    expect(document.documentElement.lang).toBe("en");
    fireEvent.click(screen.getByRole("button",{name:"Save"}));
    expect(await screen.findByRole("heading",{name:"環境設定"})).toBeVisible();
    expect(document.documentElement.lang).toBe("ja");
    expect(ipc.invoke).toHaveBeenCalledWith("save_settings",{settings:expect.objectContaining({locale:"ja"})});
  });
});
