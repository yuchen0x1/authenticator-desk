import { useEffect, useState } from "react";
import { activityCommands } from "../lib/tauri";
import type { Dashboard } from "../lib/types";
import { getMessages } from "../i18n";
import { workspaceMessages } from "../i18n/workspace";
import type { WorkspaceProps } from "../App";

export function DashboardPage({ locale, accounts, categories, navigate, onAdd, onEdit, onError }: WorkspaceProps) {
  const t = getMessages(locale), w = workspaceMessages(locale);
  const [summary, setSummary] = useState<Dashboard | null>(null);
  const [loading, setLoading] = useState(true), [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setSummary(null);
    setLoading(true);
    setFailed(false);
    activityCommands.dashboard().then((value) => {
      if (active) setSummary(value);
    }).catch((error) => {
      if (active) {
        if ((error as { code?: string })?.code === "vault_locked") onError(error);
        else setFailed(true);
      }
    }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [accounts, categories, attempt, onError]);

  return <section>
    <div className="page-heading"><div><h2>{t.pages.dashboard}</h2><p>{t.pages.dashboardHint}</p></div><button className="button button-primary" onClick={onAdd}>＋ {t.actions.add}</button></div>
    {loading && <p className="workspace-feedback" role="status">{t.pages.loading}</p>}
    {failed && <div className="workspace-feedback"><p role="alert">{w.dashboardLoadError}</p><button className="button button-subtle" onClick={() => setAttempt(value => value + 1)}>{w.retry}</button></div>}
    <div className="stats-strip" aria-busy={loading}>
      <div><span>{t.pages.accounts}</span><strong>{summary?.account_count ?? accounts.length}</strong></div>
      <div><span>{t.pages.categories}</span><strong>{summary?.category_count ?? categories.length}</strong></div>
      <div><span>{t.navigation.favorites}</span><strong>{summary?.favorite_count ?? accounts.filter((item) => item.favorite).length}</strong></div>
      <div><span>{w.copiesToday}</span><strong aria-label={summary ? undefined : loading ? t.pages.loading : w.unavailable}>{summary?.copies_today ?? "—"}</strong></div>
    </div>
    <div className="section-heading"><h3>{t.navigation.codes}</h3><button className="text-button" onClick={() => navigate("codes")}>{w.openCodes} ›</button></div>
    {accounts.length === 0 ? <div className="dashboard-empty"><div><strong>{t.pages.empty}</strong><p>{t.pages.emptyHint}</p></div><button className="button button-subtle" onClick={() => navigate("import")}>{t.actions.import}</button></div>
      : <div className="recent-list">{(summary?.recent_accounts ?? accounts.slice(0, 5)).map((item) => <button key={item.id} className="recent-row" aria-label={`${t.actions.edit}: ${item.issuer} · ${item.account_name}`} onClick={() => onEdit(item)}>
        <span className="service-mark" aria-hidden="true">{item.issuer.slice(0, 1).toUpperCase()}</span><span className="min-w-0 flex-1 text-left"><strong className="block workspace-wrap">{item.issuer}</strong><small className="block workspace-wrap text-[var(--muted)]">{item.account_name}</small></span><span className="text-[var(--muted)]" aria-hidden="true">›</span>
      </button>)}</div>}
    <div className="section-heading mt-8"><h3>{t.pages.security}</h3><button className="text-button" onClick={() => navigate("security")}>{w.openSecurity} ›</button></div>
    <p className="status-row workspace-wrap">{w.securitySummaryHint}</p>
  </section>;
}
