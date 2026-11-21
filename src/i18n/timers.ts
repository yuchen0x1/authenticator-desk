import type { Locale } from "./index";

const messages = {
  en: { disableIdle: "Disable automatic locking", disableClipboard: "Disable clipboard cleanup" },
  "zh-CN": { disableIdle: "关闭自动锁定", disableClipboard: "关闭剪贴板自动清理" },
  "zh-TW": { disableIdle: "關閉自動鎖定", disableClipboard: "關閉剪貼簿自動清理" },
  ja: { disableIdle: "自動ロックを無効にする", disableClipboard: "クリップボードの自動消去を無効にする" },
};
export const timerMessages = (locale: Locale) => messages[locale] ?? messages["zh-CN"];
