import { describe, expect, it } from "vitest";
import { getMessages, resources } from "./index";

describe("locale resources", () => {
  it("keeps all supported locales structurally complete", () => {
    const shape = (value: unknown): unknown => value && typeof value === "object" ? Object.fromEntries(Object.entries(value).sort().map(([key, child]) => [key, shape(child)])) : typeof value;
    const signatures = Object.values(resources).map((messages) => JSON.stringify(shape(messages)));
    expect(new Set(signatures).size).toBe(1);
    for (const messages of Object.values(resources)) {
      expect(messages.actions.save).toBeTruthy();
      expect(messages.navigation.settings).toBeTruthy();
      expect(messages.fields.secret).toBeTruthy();
    }
  });

  it("falls back to simplified Chinese for unknown locales", () => {
    expect(getMessages("unknown").shell.brand).toBe(getMessages("zh-CN").shell.brand);
  });
});
