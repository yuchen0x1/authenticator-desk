import type { AccountView } from "../lib/types";

const messages = {
  "zh-CN": "尚未生效",
  "zh-TW": "尚未生效",
  en: "Not active yet",
  ja: "まだ有効ではありません",
} as const;

type Locale = keyof typeof messages;

export function futureT0Message(locale: string): string {
  return messages[locale as Locale] ?? messages["zh-CN"];
}

// AccountCard receives flattened labels rather than the locale itself.
export function futureT0MessageForCopyLabel(copyLabel: string): string {
  if (copyLabel === "複製驗證碼") return messages["zh-TW"];
  if (copyLabel === "Copy code" || copyLabel === "Copy") return messages.en;
  if (copyLabel === "コードをコピー") return messages.ja;
  return messages["zh-CN"];
}

export function isFutureTotp(
  account: Pick<AccountView, "kind" | "t0">,
  unixSeconds: number | bigint = Math.floor(Date.now() / 1000),
): boolean {
  if (account.kind !== "totp") return false;
  try {
    if (!/^-?[0-9]+$/.test(account.t0)) return false;
    // Account t0 is an i64 decimal string. BigInt avoids losing precision
    // when a valid timestamp is outside JavaScript's safe integer range.
    const t0 = BigInt(account.t0);
    if (t0 < -(2n ** 63n) || t0 > 2n ** 63n - 1n) return false;
    return t0 > BigInt(unixSeconds);
  } catch {
    // Invalid account data is handled by the backend; do not label it as a
    // valid future configuration in the display layer.
    return false;
  }
}
