import { tauriInvoke } from "./tauri";

export type RestoreMode = "merge" | "replace";
export interface BackupSummary { path:string;account_count:number;category_count:number;bytes:number;created_at:string }
export interface RestorePreview { preview_id:string;mode:RestoreMode;account_count:number;category_count:number;existing_count:number;items:{index:number;issuer:string;account_name:string;status:string}[] }
export interface AutoBackupStatus { enabled:boolean;paused_locked:boolean;interval_hours:number;retention:number;last_run:number|null }
export interface BackupFileView { name:string;bytes:number;modified_at:string;automatic:boolean }
export interface SecurityAudit { checked_at:string;locked:boolean;integrity:{ok:boolean;user_version:number};corrupt_record_count:number;duplicate_count:number;invalid_config_count:number;plaintext_file_findings:string[];permission_findings:string;backup_count:number;last_backup_at:string|null;backup_configured:boolean;time_status:string }
export const backupCommands = {
  create:(password:string)=>tauriInvoke<BackupSummary|null>("backup_create",{password}),
  preview:(password:string,mode:RestoreMode)=>tauriInvoke<RestorePreview|null>("backup_preview_restore",{password,mode}),
  restore:(previewId:string,accountIndices:number[],confirmReplace:boolean)=>tauriInvoke<{restored_accounts:number;restored_categories:number;mode:RestoreMode}>("backup_apply_restore",{previewId,selection:{account_indices:accountIndices,confirm_replace:confirmReplace}}),
  status:()=>tauriInvoke<AutoBackupStatus>("backup_auto_status"),
  configure:(enabled:boolean,password:string,intervalHours:number,retention:number)=>tauriInvoke<AutoBackupStatus|null>("backup_configure_auto",{enabled,password,intervalHours,retention}),
  list:()=>tauriInvoke<BackupFileView[]|null>("backup_list"),
  audit:()=>tauriInvoke<SecurityAudit>("security_audit"),
};
