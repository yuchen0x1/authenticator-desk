import { beforeEach, describe, expect, it, vi } from "vitest";
import { cacheTheme, readCachedTheme } from "./uiPrefs";

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
