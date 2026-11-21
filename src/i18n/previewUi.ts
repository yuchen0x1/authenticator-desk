import type { Locale } from "./index";

export interface PreviewUiMessages {
  importPreviewBusy: string;
  importFileBusy: string;
  importQrBusy: string;
  importCommitBusy: string;
  backupCreateBusy: string;
  backupPreviewBusy: string;
  backupApplyBusy: string;
  backupBrowseBusy: string;
  importPreviewReady: (count: number) => string;
  backupPreviewReady: (count: number) => string;
  importPreviewExpired: string;
}

const messages: Record<Locale, PreviewUiMessages> = {
  "zh-CN": {
    importPreviewBusy: "正在生成导入预览…",
    importFileBusy: "正在读取迁移文件…",
    importQrBusy: "正在读取二维码…",
    importCommitBusy: "正在导入所选账户…",
    backupCreateBusy: "正在创建加密备份…",
    backupPreviewBusy: "正在打开备份选择器…",
    backupApplyBusy: "正在恢复所选账户…",
    backupBrowseBusy: "正在读取备份文件夹…",
    importPreviewExpired: "导入预览已失效，请重新选择文件或再次生成预览。",
    importPreviewReady: (count) => `导入预览已准备好，共 ${count} 项。`,
    backupPreviewReady: (count) => `备份预览已准备好，共 ${count} 个账户。`,
  },
  "zh-TW": {
    importPreviewBusy: "正在產生匯入預覽…",
    importFileBusy: "正在讀取遷移檔案…",
    importQrBusy: "正在讀取 QR 圖片…",
    importCommitBusy: "正在匯入所選帳戶…",
    backupCreateBusy: "正在建立加密備份…",
    backupPreviewBusy: "正在開啟備份選擇器…",
    backupApplyBusy: "正在還原所選帳戶…",
    backupBrowseBusy: "正在讀取備份資料夾…",
    importPreviewExpired: "匯入預覽已失效，請重新選擇檔案或再次產生預覽。",
    importPreviewReady: (count) => `匯入預覽已準備好，共 ${count} 項。`,
    backupPreviewReady: (count) => `備份預覽已準備好，共 ${count} 個帳戶。`,
  },
  en: {
    importPreviewBusy: "Preparing import preview…",
    importFileBusy: "Reading migration file…",
    importQrBusy: "Reading QR image…",
    importCommitBusy: "Importing selected accounts…",
    backupCreateBusy: "Creating encrypted backup…",
    backupPreviewBusy: "Opening backup picker…",
    backupApplyBusy: "Restoring selected accounts…",
    backupBrowseBusy: "Reading backup folder…",
    importPreviewExpired: "The import review expired. Choose the file again or generate a new preview.",
    importPreviewReady: (count) => `Import preview ready: ${count} item${count === 1 ? "" : "s"}.`,
    backupPreviewReady: (count) => `Backup preview ready: ${count} account${count === 1 ? "" : "s"}.`,
  },
  ja: {
    importPreviewBusy: "インポートのプレビューを準備中…",
    importFileBusy: "移行ファイルを読み込み中…",
    importQrBusy: "QR画像を読み込み中…",
    importCommitBusy: "選択したアカウントをインポート中…",
    backupCreateBusy: "暗号化バックアップを作成中…",
    backupPreviewBusy: "バックアップ選択画面を開いています…",
    backupApplyBusy: "選択したアカウントを復元中…",
    backupBrowseBusy: "バックアップフォルダーを読み込み中…",
    importPreviewExpired: "インポートのプレビューは無効になりました。ファイルを選び直すか、もう一度プレビューしてください。",
    importPreviewReady: (count) => `インポートのプレビューを準備しました（${count} 件）。`,
    backupPreviewReady: (count) => `バックアップのプレビューを準備しました（${count} アカウント）。`,
  },
};

export function getPreviewUi(locale: Locale): PreviewUiMessages {
  return messages[locale] ?? messages["zh-CN"];
}
