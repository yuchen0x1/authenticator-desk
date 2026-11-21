// The appearance choice is stored inside the encrypted vault settings, which are
// unreadable while the vault is locked. The page therefore remembers the last
// applied theme locally so the lock screen and the first paint match the saved
// preference instead of flashing the default dark theme.
const THEME_KEY = "authdesk.theme";

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
