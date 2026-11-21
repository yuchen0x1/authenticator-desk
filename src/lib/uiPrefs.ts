import type { Locale } from "../i18n";

// These hints are only for the first paint while the encrypted vault is locked.
// Authenticated settings remain authoritative once the vault is unlocked.
const THEME_KEY = "authdesk.theme";
const LOCALE_KEY = "authdesk.locale";
const DEFAULT_LOCALE: Locale = "zh-CN";
const LOCALES = ["zh-CN", "zh-TW", "en", "ja"] as const satisfies readonly Locale[];

export function isSupportedLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

export function readCachedTheme(): "light" | "dark" {
  try {
    return localStorage.getItem(THEME_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export function cacheTheme(theme: string) {
  try {
    localStorage.setItem(THEME_KEY, theme === "light" ? "light" : "dark");
  } catch {
    // A blocked storage area only costs the cached first paint, never the theme.
  }
}

export function readCachedLocale(): Locale {
  try {
    const value = localStorage.getItem(LOCALE_KEY);
    return value && isSupportedLocale(value) ? value : DEFAULT_LOCALE;
  } catch {
    return DEFAULT_LOCALE;
  }
}

export function cacheLocale(locale: string) {
  if (!isSupportedLocale(locale)) return;
  try {
    localStorage.setItem(LOCALE_KEY, locale);
  } catch {
    // A blocked storage area only costs the cached first paint, never the locale.
  }
}
