import { describe, expect, it } from "vitest";
import type { Locale } from "./index";
import { importErrorLabel, importFormatLabel } from "./importUi";

const locales: Locale[] = ["en", "zh-CN", "zh-TW", "ja"];
const formats = ["otpauth_text", "otpauth_list", "google_migration", "aegis", "andotp", "twofas", "ente_encrypted", "freeotp"];
const errors = ["invalid", "invalid_otpauth", "invalid_parameters", "missing_counter", "missing_info", "missing_secret", "unsupported_algorithm", "unsupported_digits", "unsupported_steam", "unsupported_type", "unsupported_token_type"];

describe("import labels", () => {
  it("localizes all supported formats and parse errors", () => {
    for (const locale of locales) {
      for (const format of formats) expect(importFormatLabel(locale, format), `${locale}/${format}`).not.toBe(format);
      for (const error of errors) expect(importErrorLabel(locale, error), `${locale}/${error}`).not.toBe(error);
    }
  });

  it.each([
    ["en", "Unsupported number of code digits"],
    ["zh-CN", "不支持的验证码位数"],
    ["zh-TW", "不支援的驗證碼位數"],
    ["ja", "未対応のコード桁数"],
  ] as const)("describes unsupported code digits precisely in %s", (locale, label) => {
    expect(importErrorLabel(locale, "unsupported_digits")).toBe(label);
  });

  it("preserves identifiers from newer backend versions", () => {
    expect(importFormatLabel("en", "future_format")).toBe("future_format");
    expect(importErrorLabel("ja", "future_error")).toBe("future_error");
  });
});
