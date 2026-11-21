import { describe, expect, it } from "vitest";
import { activityLabel } from "./workspace";
import type { Locale } from "./index";

// Mirrors every string the backend passes to `audit()` in
// src-tauri/src/accounts/service.rs (and the import/backup modules). If a new
// event type is added there, this list must grow with a label for each locale.
const auditCodes = [
  "account_created", "account_deleted", "account_updated", "accounts_imported",
  "accounts_moved", "accounts_reordered", "backup_restored", "category_deleted",
  "category_saved", "code_copied", "hotp_advanced", "settings_saved",
] as const;
const locales: Locale[] = ["en", "zh-CN", "zh-TW", "ja"];

describe("activity labels", () => {
  it("labels every backend audit code in every locale", () => {
    for (const locale of locales) {
      for (const code of auditCodes) {
        const label = activityLabel(locale, code);
        expect(label, `${locale}/${code}`).not.toBe(code);
        expect(label.trim()).not.toBe("");
      }
    }
  });

  it("keeps the raw code for event types this build does not know", () => {
    for (const locale of locales) expect(activityLabel(locale, "vault_unlocked")).toBe("vault_unlocked");
  });

  it("falls back to simplified Chinese for an unsupported locale", () => {
    expect(activityLabel("unknown" as Locale, "code_copied")).toBe(activityLabel("zh-CN", "code_copied"));
  });
});
