export interface AppSnapshot { locked: boolean; vault_exists: boolean; locale: string; theme: string }
export interface AppErrorPayload { code: string; message: string }
export type OtpAlgorithm = "SHA1" | "SHA256" | "SHA512";
export type OtpKind = "totp" | "hotp";
export interface AccountInput { issuer:string; account_name:string; secret?:string|null; algorithm:OtpAlgorithm; digits:number; kind:OtpKind; period_secs:number; t0:string; hotp_counter:string; category_id:string|null; tags:string[]; notes:string; favorite:boolean }
export interface AccountQuery { search:string; category_id?:string|null; tag?:string|null; favorite_only:boolean; sort:"manual"|"favorites"|"issuer" }
export interface AccountView { id:string; issuer:string; account_name:string; algorithm:OtpAlgorithm; digits:number; kind:OtpKind; period_secs:number; t0:string; hotp_counter:string; category_id:string|null; tags:string[]; notes_present:boolean; favorite:boolean; sort_order:number; created_at:string; updated_at:string }
export interface AccountDetails { account:AccountView; notes:string }
export interface CodeView {account_id:string;code:string;remaining_secs:number;progress:number;generated_at:number;clock_offset_secs:number;wall_clock_jump:boolean}
export interface CategoryView {id:string;name:string;color:string}
export interface AuditEvent {id:number;event_type:string;account_id:string|null;created_at:string}
export interface Dashboard {account_count:number;category_count:number;favorite_count:number;copies_today:number;recent_accounts:AccountView[]}
export interface AppSettings {locale:string;theme:string;idle_timeout_secs:number;clipboard_ttl_secs:number;hide_codes:boolean;view_mode:string;animations:boolean;compact:boolean;font_size:number;default_algorithm:OtpAlgorithm;default_digits:number;default_period_secs:number;close_to_tray:boolean;minimize_to_tray:boolean;startup:boolean;backup_enabled:boolean;backup_interval_hours:number;backup_retention:number;show_next_code:boolean}
export interface ImportItem {index:number;issuer:string|null;account_name:string|null;status:"new"|"duplicate"|"invalid";error_code:string|null}
export interface ImportPreview {preview_id:string;format:string;items:ImportItem[]}
export interface PlatformTemplate {issuer:string;icon:string;category:string;help_key:string}
