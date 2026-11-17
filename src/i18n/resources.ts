export interface Messages {
  navigation: { label: string; items: { dashboard: string; codes: string; accounts: string; favorites: string; categories: string; import: string; backup: string; security: string; activity: string; settings: string } };
  shell: { workspace: string; localOnly: string; lockedExplanation: string; nextStep: string };
  status: { checkingTitle: string; checkingDetail: string; unavailableTitle: string; unavailableDetail: string; lockedTitle: string; lockedDetail: string; noVaultDetail: string; readyTitle: string };
}

export const resources = {
  "zh-CN": {
    navigation: { label: "主导航", items: { dashboard: "仪表盘", codes: "验证码中心", accounts: "所有账户", favorites: "收藏账户", categories: "分类管理", import: "导入迁移", backup: "备份恢复", security: "安全中心", activity: "操作日志", settings: "设置" } },
    shell: { workspace: "本地安全工作区", localOnly: "仅在本机运行", lockedExplanation: "账户和验证码仅在保险库解锁后显示。此锁定界面不会读取或展示任何账户数据。", nextStep: "保险库创建与解锁将在安全存储模块接入后启用。" },
    status: { checkingTitle: "正在连接桌面安全后端", checkingDetail: "正在确认本地保险库状态。", unavailableTitle: "桌面安全后端不可用", unavailableDetail: "请在 AUTHENTICATOR DESK 桌面应用中打开此界面。", lockedTitle: "保险库已锁定", lockedDetail: "解锁后才能读取本地账户。", noVaultDetail: "尚未创建本地保险库。", readyTitle: "保险库已就绪" },
  },
  "zh-TW": {
    navigation: { label: "主導覽", items: { dashboard: "儀表板", codes: "驗證碼中心", accounts: "所有帳戶", favorites: "收藏帳戶", categories: "分類管理", import: "匯入遷移", backup: "備份還原", security: "安全中心", activity: "操作記錄", settings: "設定" } },
    shell: { workspace: "本機安全工作區", localOnly: "僅在本機執行", lockedExplanation: "帳戶與驗證碼只會在保險庫解鎖後顯示。此鎖定畫面不會讀取或顯示任何帳戶資料。", nextStep: "保險庫建立與解鎖將在安全儲存模組接入後啟用。" },
    status: { checkingTitle: "正在連接桌面安全後端", checkingDetail: "正在確認本機保險庫狀態。", unavailableTitle: "桌面安全後端無法使用", unavailableDetail: "請在 AUTHENTICATOR DESK 桌面應用程式中開啟此介面。", lockedTitle: "保險庫已鎖定", lockedDetail: "解鎖後才能讀取本機帳戶。", noVaultDetail: "尚未建立本機保險庫。", readyTitle: "保險庫已就緒" },
  },
  en: {
    navigation: { label: "Primary navigation", items: { dashboard: "Dashboard", codes: "Codes", accounts: "All accounts", favorites: "Favorites", categories: "Categories", import: "Import", backup: "Backup", security: "Security", activity: "Activity", settings: "Settings" } },
    shell: { workspace: "Local secure workspace", localOnly: "Runs only on this device", lockedExplanation: "Accounts and codes appear only after the vault is unlocked. This locked view does not read or display account data.", nextStep: "Vault creation and unlocking will be enabled when secure storage is connected." },
    status: { checkingTitle: "Connecting to the desktop security backend", checkingDetail: "Checking the local vault state.", unavailableTitle: "Desktop security backend unavailable", unavailableDetail: "Open this interface in the AUTHENTICATOR DESK desktop app.", lockedTitle: "Vault locked", lockedDetail: "Unlock the vault to read local accounts.", noVaultDetail: "No local vault has been created.", readyTitle: "Vault ready" },
  },
  ja: {
    navigation: { label: "メインナビゲーション", items: { dashboard: "ダッシュボード", codes: "認証コード", accounts: "すべてのアカウント", favorites: "お気に入り", categories: "カテゴリ", import: "インポート", backup: "バックアップ", security: "セキュリティ", activity: "操作ログ", settings: "設定" } },
    shell: { workspace: "ローカル安全ワークスペース", localOnly: "この端末だけで実行", lockedExplanation: "アカウントと認証コードは保管庫のロック解除後にのみ表示されます。この画面はアカウントデータを読み取りません。", nextStep: "安全なストレージ接続後に保管庫の作成とロック解除を有効にします。" },
    status: { checkingTitle: "デスクトップ安全バックエンドに接続中", checkingDetail: "ローカル保管庫の状態を確認しています。", unavailableTitle: "デスクトップ安全バックエンドを利用できません", unavailableDetail: "AUTHENTICATOR DESK デスクトップアプリでこの画面を開いてください。", lockedTitle: "保管庫はロックされています", lockedDetail: "ローカルアカウントを読むにはロックを解除してください。", noVaultDetail: "ローカル保管庫はまだ作成されていません。", readyTitle: "保管庫の準備ができました" },
  },
} satisfies Record<string, Messages>;

export type SupportedLocale = keyof typeof resources;
export function getMessages(locale: string): Messages { return resources[locale as SupportedLocale] ?? resources["zh-CN"]; }
