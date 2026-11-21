import { resources, type Locale } from "./index";

const messages = {
  "zh-CN": {
    orderTitle: "账户排序", reorderFavorites: "请在所有账户页面调整顺序。",
    secretReplacementRequired: "新密钥已在提交时安全清空。再次保存前，请重新输入新密钥，或取消替换以保留原密钥。",
    cancelSecretReplacement: "取消替换，保留原密钥",
  },
  "zh-TW": {
    orderTitle: "帳戶排序", reorderFavorites: "請在所有帳戶頁面調整順序。",
    secretReplacementRequired: "新密鑰已在提交時安全清空。再次儲存前，請重新輸入新密鑰，或取消替換以保留原密鑰。",
    cancelSecretReplacement: "取消替換，保留原密鑰",
  },
  en: {
    orderTitle: "Account order", reorderFavorites: "Reorder from the All accounts page.",
    secretReplacementRequired: "The new secret was cleared for security on submission. Before saving again, re-enter the new secret or cancel replacement to keep the current secret.",
    cancelSecretReplacement: "Cancel replacement and keep current secret",
  },
  ja: {
    orderTitle: "アカウントの並び順", reorderFavorites: "すべてのアカウント画面で並べ替えてください。",
    secretReplacementRequired: "送信時に安全のため新しい秘密鍵を消去しました。再保存する前に、新しい秘密鍵を再入力するか、置換を取り消して現在の秘密鍵を維持してください。",
    cancelSecretReplacement: "置換を取り消して現在の秘密鍵を維持",
  },
};

export const accountUiMessages = (locale: Locale) => messages[locale] ?? messages["zh-CN"];

// AccountDialog receives flattened translations, without a separate locale prop.
export function accountDialogMessages(secretKeep: string) {
  const locale = (Object.keys(messages) as Locale[]).find((locale) => resources[locale].fields.secretKeep === secretKeep) ?? "zh-CN";
  return accountUiMessages(locale);
}
