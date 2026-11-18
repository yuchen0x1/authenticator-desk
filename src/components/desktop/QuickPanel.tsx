import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { accountCommands, desktopCommands, settingsCommands, vaultCommands } from "../../lib/tauri";
import type { AccountView, CodeView } from "../../lib/types";
import { getMessages } from "../../i18n";

export function QuickPanel() {
  const [locale, setLocale] = useState("zh-CN");
  const t = getMessages(locale);
  const [accounts, setAccounts] = useState<AccountView[]>([]);
  const [codes, setCodes] = useState<CodeView[]>([]);
  const [search, setSearch] = useState("");
  const [copied, setCopied] = useState("");
  const [copyError, setCopyError] = useState(false);
  const [locked, setLocked] = useState(true);
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    let live = true;
    let inFlight = false;
    let generation = 0;
    let settingsRevision: number | undefined;
    let settingsGeneration: number | undefined;
    const load = async () => {
      if (!live || inFlight) return;
      inFlight = true;
      const currentGeneration = generation;
      try {
        const status = await vaultCommands.status();
        if (!live || currentGeneration !== generation) return;
        if (status.locked) {
          setLocked(true);
          setAccounts([]);
          setCodes([]);
          return;
        }
        if (status.generation !== settingsGeneration || status.revision !== settingsRevision) {
          const settings = await settingsCommands.get();
          if (!live || currentGeneration !== generation) return;
          setLocale(settings.locale);
          settingsGeneration = status.generation;
          settingsRevision = status.revision;
        }
        const values = await desktopCommands.quickPanelAccounts();
        if (!live || currentGeneration !== generation) return;
        const nextCodes = values.length ? await accountCommands.codes(values.map((item) => item.id)) : [];
        const latestStatus = await vaultCommands.status();
        if (!live || currentGeneration !== generation) return;
        if (latestStatus.locked || latestStatus.generation !== status.generation) {
          setLocked(true);
          setAccounts([]);
          setCodes([]);
          return;
        }
        setAccounts(values);
        setCodes(nextCodes);
        setLocked(false);
      } catch {
        if (live && currentGeneration === generation) {
          setLocked(true);
          setAccounts([]);
          setCodes([]);
        }
      } finally {
        inFlight = false;
      }
    };
    const onVisibilityChange = () => { if (document.visibilityState === "visible") void load(); };
    void load();
    const timer = window.setInterval(() => void load(), 1000);
    const clock = window.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    window.addEventListener("focus", load);
    document.addEventListener("visibilitychange", onVisibilityChange);
    let stop: (() => void) | undefined;
    void listen("vault://locked", () => {
      generation++;
      setLocked(true);
      setAccounts([]);
      setCodes([]);
      setCopied("");
      setCopyError(false);
    }).then((unlisten) => { if (live) stop = unlisten; else unlisten(); }).catch(() => {});
    return () => { live = false; generation++; window.clearInterval(timer); window.clearInterval(clock); window.removeEventListener("focus", load); document.removeEventListener("visibilitychange", onVisibilityChange); stop?.(); };
  }, []);

  const visible = useMemo(() => accounts.filter((item) => `${item.issuer} ${item.account_name}`.toLowerCase().includes(search.toLowerCase())), [accounts, search]);
  const byId = new Map(codes.map((item) => [item.account_id, item]));
  async function copy(account: AccountView) {
    setCopyError(false);
    try {
      await accountCommands.copy(account.id);
      setCopied(account.id);
      window.setTimeout(() => setCopied((value) => value === account.id ? "" : value), 1500);
    } catch (error) {
      const code = (error as { code?: string })?.code;
      if (code === "vault_locked") {
        setLocked(true);
        setAccounts([]);
        setCodes([]);
      } else {
        setCopyError(true);
      }
    }
  }

  return <main className="quick-panel-shell">
    <header className="quick-panel-header"><div><strong>{t.shell.brand}</strong><span>{t.pages.favorites}</span></div><button className="icon-button" onClick={() => void desktopCommands.showMain()} aria-label={t.desktop.openMain}>↗</button></header>
    <label className="search-field quick-panel-search"><span aria-hidden="true">⌕</span><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t.fields.search} aria-label={t.fields.search} autoFocus /></label>
    {copyError && <p className="quick-panel-error" role="alert">{t.desktop.copyError}</p>}
    {locked ? <div className="quick-panel-empty">{t.shell.locked}</div> : visible.length === 0 ? <div className="quick-panel-empty">{t.pages.empty}</div> : <div className="quick-panel-list">{visible.map((account) => { const code = byId.get(account.id); const elapsed = code ? Math.max(0, Math.floor(now - code.generated_at)) : 0; const remaining = code && account.kind === "totp" ? Math.max(0, code.remaining_secs - elapsed) : null; return <article className="quick-panel-row" key={account.id}><div className="min-w-0 flex-1"><strong className="block truncate">{account.issuer}</strong><span className="block truncate text-xs text-[var(--muted)]">{account.account_name}</span></div><div className="quick-panel-code"><code>{code?.code ?? "------"}</code>{remaining !== null && <span>{remaining}s</span>}</div><button className="button button-subtle" disabled={!code} onClick={() => void copy(account)}>{copied === account.id ? t.status.copied : t.actions.copy}</button></article>; })}</div>}
  </main>;
}
