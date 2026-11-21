import type { Locale } from "./index";

const formatLabels: Record<Locale, Record<string, string>> = {
  en: {
    otpauth_text: "Authenticator URI text", otpauth_list: "Authenticator URI list",
    google_migration: "Google Authenticator migration", aegis: "Aegis", andotp: "andOTP",
    twofas: "2FAS", ente_encrypted: "Ente encrypted export", freeotp: "FreeOTP",
  },
  "zh-CN": {
    otpauth_text: "otpauth URI 文本", otpauth_list: "otpauth URI 列表",
    google_migration: "Google 身份验证器迁移数据", aegis: "Aegis", andotp: "andOTP",
    twofas: "2FAS", ente_encrypted: "Ente 加密导出", freeotp: "FreeOTP",
  },
  "zh-TW": {
    otpauth_text: "otpauth URI 文字", otpauth_list: "otpauth URI 清單",
    google_migration: "Google Authenticator 遷移資料", aegis: "Aegis", andotp: "andOTP",
    twofas: "2FAS", ente_encrypted: "Ente 加密匯出", freeotp: "FreeOTP",
  },
  ja: {
    otpauth_text: "otpauth URI テキスト", otpauth_list: "otpauth URI リスト",
    google_migration: "Google Authenticator の移行データ", aegis: "Aegis", andotp: "andOTP",
    twofas: "2FAS", ente_encrypted: "Ente 暗号化エクスポート", freeotp: "FreeOTP",
  },
};

const errorLabels: Record<Locale, Record<string, string>> = {
  en: {
    invalid: "Invalid import item", invalid_otpauth: "Invalid authenticator URI", invalid_parameters: "Invalid settings",
    missing_counter: "Missing counter", missing_info: "Missing account information",
    missing_secret: "Missing secret", unsupported_algorithm: "Unsupported algorithm",
    unsupported_digits: "Unsupported number of code digits",
    unsupported_steam: "Steam tokens are not supported", unsupported_type: "Unsupported token type",
    unsupported_token_type: "Unsupported token type",
  },
  "zh-CN": {
    invalid: "导入项目无效", invalid_otpauth: "无效的验证器 URI", invalid_parameters: "参数无效",
    missing_counter: "缺少计数器", missing_info: "缺少账户信息", missing_secret: "缺少密钥",
    unsupported_algorithm: "不支持的算法", unsupported_digits: "不支持的验证码位数", unsupported_steam: "不支持 Steam 令牌",
    unsupported_type: "不支持的令牌类型", unsupported_token_type: "不支持的令牌类型",
  },
  "zh-TW": {
    invalid: "匯入項目無效", invalid_otpauth: "無效的驗證器 URI", invalid_parameters: "參數無效",
    missing_counter: "缺少計數器", missing_info: "缺少帳戶資訊", missing_secret: "缺少密鑰",
    unsupported_algorithm: "不支援的演算法", unsupported_digits: "不支援的驗證碼位數", unsupported_steam: "不支援 Steam 權杖",
    unsupported_type: "不支援的權杖類型", unsupported_token_type: "不支援的權杖類型",
  },
  ja: {
    invalid: "インポート項目が無効です", invalid_otpauth: "無効な認証 URI", invalid_parameters: "設定が無効です",
    missing_counter: "カウンターがありません", missing_info: "アカウント情報がありません",
    missing_secret: "秘密鍵がありません", unsupported_algorithm: "未対応のアルゴリズム",
    unsupported_digits: "未対応のコード桁数",
    unsupported_steam: "Steam トークンには対応していません", unsupported_type: "未対応のトークン形式",
    unsupported_token_type: "未対応のトークン形式",
  },
};

export function importFormatLabel(locale: Locale, format: string) {
  return formatLabels[locale]?.[format] ?? format;
}

export function importErrorLabel(locale: Locale, error: string) {
  return errorLabels[locale]?.[error] ?? error;
}
