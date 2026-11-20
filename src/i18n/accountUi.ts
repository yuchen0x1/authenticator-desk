import type { Locale } from "./index";

const messages = {
  "zh-CN": { orderTitle: "账户排序", reorderFavorites: "请在所有账户页面调整顺序。" },
  "zh-TW": { orderTitle: "帳戶排序", reorderFavorites: "請在所有帳戶頁面調整順序。" },
  en: { orderTitle: "Account order", reorderFavorites: "Reorder from the All accounts page." },
  ja: { orderTitle: "アカウントの並び順", reorderFavorites: "すべてのアカウント画面で並べ替えてください。" },
};

export const accountUiMessages = (locale: Locale) => messages[locale] ?? messages["zh-CN"];
