import type { ReactNode } from "react";
import { getMessages, type Locale } from "../../i18n";

export type Destination = "dashboard" | "codes" | "accounts" | "favorites" | "categories" | "import" | "backup" | "security" | "activity" | "settings";
const nav: Destination[] = ["dashboard", "codes", "accounts", "favorites", "categories", "import", "backup", "security", "activity", "settings"];
const symbols: Record<Destination, string> = { dashboard: "⌂", codes: "◉", accounts: "▤", favorites: "☆", categories: "▦", import: "↧", backup: "▣", security: "◇", activity: "≋", settings: "⚙" };

export function AppShell({ locale, page, locked, onNavigate, onLock, children }: {
  locale: Locale;
  page: Destination;
  locked: boolean;
  onNavigate: (page: Destination) => void;
  onLock: () => void;
  children: ReactNode;
}) {
  const t = getMessages(locale);
  const title = t.navigation[page];
  return <div className="shell-layout">
    <aside className="sidebar">
      <div className="brand"><span className="brand-symbol">◇</span><span>{t.shell.brand}</span></div>
      <div className="sidebar-caption">{t.shell.local}</div>
      <nav aria-label={t.navigation.label}><ul>{nav.map((item) => <li key={item}>
        <button aria-current={page === item ? "page" : undefined} onClick={() => onNavigate(item)}>
          <span className="nav-symbol" aria-hidden="true">{symbols[item]}</span><span>{t.navigation[item]}</span>
        </button>
      </li>)}</ul></nav>
      <div className="sidebar-footer"><span className={`status-dot ${locked ? "" : "status-dot-live"}`} /><span>{locked ? t.shell.locked : t.shell.unlocked}</span></div>
    </aside>
    <main className="main-shell">
      <header className="topbar"><div><p className="page-context">{t.navigation[page]}</p><h1>{title}</h1></div>
        <div className="topbar-actions"><span className="vault-status"><span className={`status-dot ${locked ? "" : "status-dot-live"}`} />{locked ? t.shell.locked : t.shell.unlocked}</span>{!locked && <button className="button button-subtle" onClick={onLock}>{t.shell.lock}</button>}</div>
      </header>
      <div className="page-content">{children}</div>
    </main>
  </div>;
}
