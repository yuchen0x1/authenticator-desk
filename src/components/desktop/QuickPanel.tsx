import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { accountCommands, desktopCommands, settingsCommands, vaultCommands } from "../../lib/tauri";
import { readCachedTheme } from "../../lib/uiPrefs";
import type { AccountView, CodeView } from "../../lib/types";
import { getMessages } from "../../i18n";
import { futureT0Message, isFutureTotp } from "../../i18n/futureT0";

type PanelState = "loading" | "ready" | "locked" | "error" | "unavailable";

export function QuickPanel() {
  const [locale, setLocale] = useState("zh-CN");
  const t = getMessages(locale);
  const [theme, setTheme] = useState<string>(readCachedTheme);
  const [animations, setAnimations] = useState(true);
  const [accounts, setAccounts] = useState<AccountView[]>([]);
  const [codes, setCodes] = useState<CodeView[]>([]);
  const [search, setSearch] = useState("");
  const [copied, setCopied] = useState("");
  const [copyError, setCopyError] = useState(false);
  const [copyAnnouncement, setCopyAnnouncement] = useState("");
  const [phase, setPhase] = useState<PanelState>("loading");
  const [masked, setMasked] = useState(true);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  const generation = useRef(0);
  const live = useRef(false);
  const blocked = useRef(true);
  const locked = useRef(true);
  const retry = useRef<() => void>(() => {});
  const copyTimer = useRef<number | undefined>(undefined);
  const clearCopy = useCallback(() => {
    window.clearTimeout(copyTimer.current);
    copyTimer.current = undefined;
    setCopied(""); setCopyAnnouncement("");
  }, []);
  const clearLocked = useCallback(() => {
    generation.current++;
    blocked.current = true;
    locked.current = true;
    setPhase("locked");
    setAccounts([]); setCodes([]); clearCopy(); setCopyError(false); setSearch("");
  }, [clearCopy]);

  useLayoutEffect(() => { document.documentElement.lang = locale; }, [locale]);
  // This window is a separate document, so it must publish the theme itself or
  // the panel keeps the dark tokens while the main window is light.
  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.motion = animations ? "on" : "off";
  }, [theme, animations]);
  useEffect(() => {
    live.current = true;
    let disposed = false;
    let request = 0;
    let inFlight: number | undefined;
    let settingsRevision: number | undefined;
    let settingsGeneration: number | undefined;
    const invalidateCodes = () => {
      generation.current++;
      blocked.current = true;
      setMasked(true); setCodes([]); clearCopy(); setCopyError(false);
    };
    const load = async (force = false) => {
      if (disposed || (!force && inFlight !== undefined)) return;
      const currentRequest = ++request;
      inFlight = currentRequest;
      const token = generation.current;
      const current = () => !disposed && token === generation.current && currentRequest === request;
      try {
        const status = await vaultCommands.status();
        if (!current()) return;
        if (status.locked) { clearLocked(); return; }
        if (status.generation !== settingsGeneration || status.revision !== settingsRevision) {
          blocked.current = true;
          setMasked(true); setCodes([]); clearCopy();
          const settings = await settingsCommands.get();
          if (!current()) return;
          setLocale(settings.locale);
          setTheme(settings.theme === "light" ? "light" : "dark");
          setAnimations(settings.animations !== false);
          blocked.current = settings.hide_codes;
          setMasked(settings.hide_codes);
          settingsGeneration = status.generation;
          settingsRevision = status.revision;
        }
        const values = await desktopCommands.quickPanelAccounts();
        if (!current()) return;
        const nextCodes = !blocked.current && values.length ? await accountCommands.codes(values.map((item) => item.id)) : [];
        if (!current()) return;
        const latestStatus = await vaultCommands.status();
        if (!current()) return;
        if (latestStatus.locked) { clearLocked(); return; }
        if (latestStatus.generation !== status.generation || latestStatus.revision !== status.revision) {
          settingsRevision = undefined; settingsGeneration = undefined;
          invalidateCodes();
          return;
        }
        setAccounts(values);
        setCodes(blocked.current ? [] : nextCodes);
        locked.current = false;
        setPhase("ready");
      } catch (reason) {
        if (!current()) return;
        if ((reason as { code?: string })?.code === "vault_locked") { clearLocked(); return; }
        blocked.current = true;
        setCodes([]); clearCopy(); setCopyError(false);
        setPhase((reason as { code?: string })?.code === "backend_unavailable" ? "unavailable" : "error");
        settingsRevision = undefined; settingsGeneration = undefined;
      } finally {
        if (inFlight === currentRequest) inFlight = undefined;
      }
    };
    const onFocus = () => { void load(); };
    const onVisibilityChange = () => { if (document.visibilityState === "visible") void load(); };
    const onSettingsChanged = () => {
      // Revoke any in-flight response before asking which visibility is now saved.
      invalidateCodes();
      settingsRevision = undefined; settingsGeneration = undefined;
      void load(true);
    };
    retry.current = () => { setPhase("loading"); void load(true); };
    void load();
    const timer = window.setInterval(() => void load(), 1000);
    let clockTimer: number;
    const clock = () => {
      setNow(Math.floor(Date.now() / 1000));
      clockTimer = window.setTimeout(clock, 1000 - Date.now() % 1000);
    };
    clock();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);
    const stops: (() => void)[] = [];
    for (const [event, handler] of [["vault://locked", clearLocked], ["settings://changed", onSettingsChanged]] as const) {
      void listen(event, handler).then((stop) => { if (disposed) stop(); else stops.push(stop); }).catch(() => {});
    }
    return () => {
      disposed = true; live.current = false; generation.current++; blocked.current = true;
      window.clearInterval(timer); window.clearTimeout(clockTimer); window.clearTimeout(copyTimer.current);
      window.removeEventListener("focus", onFocus); document.removeEventListener("visibilitychange", onVisibilityChange);
      stops.forEach((stop) => stop());
    };
  }, [clearCopy, clearLocked]);

  const visible = useMemo(() => accounts.filter((item) => `${item.issuer} ${item.account_name}`.toLowerCase().includes(search.toLowerCase())), [accounts, search]);
  const byId = new Map(codes.map((item) => [item.account_id, item]));
  function usableCode(account: AccountView) {
    const currentUnix = Math.max(now, Math.floor(Date.now() / 1000));
    const code = byId.get(account.id);
    const futureT0 = isFutureTotp(account, currentUnix - (code?.clock_offset_secs ?? 0));
    const remaining = code ? Math.max(0, code.remaining_secs - Math.max(0, currentUnix - code.clock_offset_secs - code.generated_at)) : 0;
    return { code: !masked && !futureT0 && code && (account.kind === "hotp" || remaining > 0) ? code : undefined, remaining, futureT0 };
  }
  async function copy(account: AccountView) {
    const currentCode = usableCode(account).code;
    if (locked.current || blocked.current || !currentCode) return;
    const token = generation.current;
    setCopyError(false);
    try {
      await accountCommands.copy(account.id, currentCode.code);
      if (!live.current || token !== generation.current || locked.current || blocked.current) return;
      clearCopy();
      setCopied(account.id);
      setCopyAnnouncement(`${t.status.copied}: ${account.issuer} · ${account.account_name}`);
      copyTimer.current = window.setTimeout(() => {
        if (live.current && token === generation.current) clearCopy();
      }, 1500);
    } catch (reason) {
      if (!live.current || token !== generation.current) return;
      if ((reason as { code?: string })?.code === "vault_locked") clearLocked();
      else {
        setCopyError(true);
        if ((reason as { code?: string })?.code === "conflict") {
          setCodes([]); clearCopy(); retry.current();
        }
      }
    }
  }

  return <main className="quick-panel-shell" data-theme={theme}>
    <header className="quick-panel-header"><div><h1>{t.shell.brand}</h1><span>{t.pages.favorites}</span></div><button className="icon-button" onClick={() => void desktopCommands.showMain()} aria-label={t.desktop.openMain} title={t.desktop.openMain}>↗</button></header>
    <label className="search-field quick-panel-search"><span aria-hidden="true">⌕</span><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t.fields.search} aria-label={t.fields.search} autoFocus /></label>
    {copyError && <p className="quick-panel-error" role="alert">{t.desktop.copyError}</p>}
    <p className="visually-hidden" role="status" aria-live="polite">{copyAnnouncement}</p>
    {phase === "loading" ? <div className="quick-panel-empty" role="status">{t.shell.checking}</div> : phase === "locked" ? <div className="quick-panel-empty" role="status">{t.shell.locked}</div> : phase === "error" || phase === "unavailable" ? <div className="quick-panel-empty"><p role="alert">{phase === "unavailable" ? t.shell.unavailable : t.status.error}</p><button className="button button-subtle" onClick={() => retry.current()}>{t.desktop.retry}</button></div> : visible.length === 0 ? <div className="quick-panel-empty" role="status">{search ? t.pages.noMatch : t.pages.empty}</div> : <div className="quick-panel-list">{visible.map((account) => {
      const {code,remaining,futureT0} = usableCode(account);
      const name = `${account.issuer} ${account.account_name}`;
      return <article className="quick-panel-row" key={account.id}><div className="min-w-0 flex-1"><strong className="block truncate">{account.issuer}</strong><span className="block truncate text-xs text-[var(--muted)]">{account.account_name}</span></div><div className="quick-panel-code">{futureT0 && !masked ? <span className="future-t0-status text-sm text-[var(--muted)] workspace-wrap">{futureT0Message(locale)}</span> : <code aria-hidden={masked || undefined}>{masked ? "•".repeat(account.digits) : code?.code ?? "-".repeat(account.digits)}</code>}{masked && <span className="visually-hidden">{t.status.hiddenCode}</span>}{account.kind === "totp" && !futureT0 && <span>{code ? `${remaining}s` : "0s"}</span>}</div><button className="button button-subtle" aria-label={`${copied === account.id ? t.status.copied : t.actions.copy}: ${name}`} disabled={!code} onClick={() => void copy(account)}>{code && copied === account.id ? t.status.copied : t.actions.copy}</button></article>;
    })}</div>}
  </main>;
}
