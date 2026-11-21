import { beforeEach, describe, expect, it, vi } from "vitest";
import { cacheLocale, cacheTheme, readCachedLocale, readCachedTheme } from "./uiPrefs";

beforeEach(() => localStorage.clear());

describe("cached appearance", () => {
  it("remembers the last applied theme for the next launch", () => {
    expect(readCachedTheme()).toBe("dark");
    cacheTheme("light");
    expect(readCachedTheme()).toBe("light");
    cacheTheme("dark");
    expect(readCachedTheme()).toBe("dark");
  });

  it("normalizes unknown values instead of trusting stored text", () => {
    localStorage.setItem("authdesk.theme", "sepia");
    expect(readCachedTheme()).toBe("dark");
    cacheTheme("system");
    expect(localStorage.getItem("authdesk.theme")).toBe("dark");
  });

  it("survives a blocked storage area", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(readCachedTheme()).toBe("dark");
    expect(() => cacheTheme("light")).not.toThrow();
    getItem.mockRestore();
    setItem.mockRestore();
  });
});

describe("cached locale hint", () => {
  it.each(["zh-CN", "zh-TW", "en", "ja"] as const)("persists supported locale %s across reads", (locale) => {
    expect(readCachedLocale()).toBe("zh-CN");
    cacheLocale(locale);
    expect(localStorage.getItem("authdesk.locale")).toBe(locale);
    expect(readCachedLocale()).toBe(locale);
  });

  it.each(["", "fr", "EN", "zh-cn", "ja-JP", "undefined"])("rejects corrupted or unsupported value %j safely", (locale) => {
    localStorage.setItem("authdesk.locale", locale);
    expect(readCachedLocale()).toBe("zh-CN");
    cacheLocale(locale);
    expect(localStorage.getItem("authdesk.locale")).toBe(locale);
  });

  it("does not write unsupported values", () => {
    cacheLocale("fr");
    expect(localStorage.getItem("authdesk.locale")).toBeNull();
  });

  it("falls back safely when storage reads and writes fail", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(readCachedLocale()).toBe("zh-CN");
    expect(() => cacheLocale("ja")).not.toThrow();
    getItem.mockRestore();
    setItem.mockRestore();
  });
});
