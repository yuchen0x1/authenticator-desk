import { invoke, isTauri, type InvokeArgs } from "@tauri-apps/api/core";
import type { AccountDetails,AccountInput,AccountQuery,AccountView,AppSettings,AuditEvent,CategoryView,CodeView,Dashboard,ImportPreview,PlatformTemplate } from "./types";
import { accountCreateSchema,accountInputSchema,importFileSchema,importTextSchema } from "./schemas";

export class TauriUnavailableError extends Error {
  readonly code = "backend_unavailable";
  constructor() {
    super("The AUTHENTICATOR DESK desktop backend is unavailable.");
    this.name = "TauriUnavailableError";
  }
}

export function tauriInvoke<T>(command: string, payload?: unknown): Promise<T> {
  if (!isTauri()) return Promise.reject(new TauriUnavailableError());
  return invoke<T>(command, payload as InvokeArgs | undefined);
}
export const accountCommands = {
 list:(query:AccountQuery)=>tauriInvoke<AccountView[]>("list_accounts",{query}), details:(id:string)=>tauriInvoke<AccountDetails>("account_details",{id}),
 create:(input:AccountInput)=>tauriInvoke<AccountView>("create_account",{input:accountCreateSchema.parse(input)}), update:(id:string,input:AccountInput)=>tauriInvoke<AccountView>("update_account",{id,input:accountInputSchema.parse(input)}),
 remove:(ids:string[])=>tauriInvoke<void>("delete_accounts",{ids}), favorite:(id:string,favorite:boolean)=>tauriInvoke<void>("set_favorite",{id,favorite}),
 move:(ids:string[],categoryId:string|null)=>tauriInvoke<void>("move_accounts",{ids,categoryId}), reorder:(ids:string[])=>tauriInvoke<void>("reorder_accounts",{ids}), codes:(ids:string[])=>tauriInvoke<CodeView[]>("generate_codes",{ids}), advanceHotp:(id:string)=>tauriInvoke<CodeView>("advance_hotp",{id}),
 copy:(accountId:string)=>tauriInvoke<{token:string}>("copy_code",{accountId}),
};
export const vaultCommands = {
 status:()=>tauriInvoke<{exists:boolean;locked:boolean;generation:number;revision:number}>("vault_status"),
 create:(password:string)=>tauriInvoke<void>("create_vault",{password}),
 unlock:(password:string)=>tauriInvoke<{exists:boolean;locked:boolean;generation:number;revision:number}>("unlock_vault",{password}),
 lock:()=>tauriInvoke<void>("lock_vault"),
 integrity:()=>tauriInvoke<{ok:boolean;user_version:number}>("integrity_check"),
};
export const categoryCommands = {list:()=>tauriInvoke<CategoryView[]>("list_categories"),save:(id:string|null,name:string,color:string)=>tauriInvoke<CategoryView>("save_category",{id,name,color}),remove:(id:string)=>tauriInvoke<void>("delete_category",{id})};
export const settingsCommands = {get:()=>tauriInvoke<AppSettings>("settings"),save:(settings:AppSettings)=>tauriInvoke<void>("save_settings",{settings})};
export const activityCommands = {events:(limit=100)=>tauriInvoke<AuditEvent[]>("audit_events",{limit}),recordCopy:(accountId:string)=>tauriInvoke<void>("record_copy",{accountId}),dashboard:()=>tauriInvoke<Dashboard>("dashboard"),touch:()=>tauriInvoke<void>("activity_touch")};
export const importCommands = {preview:(text:string)=>tauriInvoke<ImportPreview>("preview_import",{text:importTextSchema.parse(text)}),previewFile:(payloadB64:string)=>tauriInvoke<ImportPreview>("preview_import_file",{payloadB64:importFileSchema.parse(payloadB64)}),commit:(previewId:string,selection:number[])=>tauriInvoke<number>("commit_import",{previewId,selection})};
export const platformTemplates = ()=>tauriInvoke<PlatformTemplate[]>("platform_templates");
export const changeMasterPassword = (current:string,next:string)=>tauriInvoke<void>("change_password",{current,next});

export interface DesktopConfig { close_to_tray:boolean; minimize_to_tray:boolean; startup:boolean; notifications:boolean; show_shortcut:string; quick_panel_shortcut:string; lock_shortcut:string }
export interface DesktopCapabilities { tray:boolean; shortcuts:boolean; startup:boolean; notifications:boolean; clipboard:boolean; qr_image:boolean; camera:boolean; camera_reason:string; warnings:string[] }
export const desktopCommands = {
 capabilities:()=>tauriInvoke<DesktopCapabilities>("desktop_capabilities"),
 config:()=>tauriInvoke<DesktopConfig>("desktop_config"),
 saveConfig:(config:DesktopConfig)=>tauriInvoke<void>("save_desktop_config",{config}),
 showQuickPanel:()=>tauriInvoke<void>("show_quick_panel"),
 showMain:()=>tauriInvoke<void>("show_main_window"),
 quickPanelAccounts:()=>tauriInvoke<AccountView[]>("quick_panel_accounts"),
 pickQrImage:()=>tauriInvoke<ImportPreview|null>("pick_qr_image"),
 testNotification:()=>tauriInvoke<void>("test_notification"),
};
