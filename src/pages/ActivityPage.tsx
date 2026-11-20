import { useEffect, useState } from "react";
import { activityCommands } from "../lib/tauri";
import type { AuditEvent } from "../lib/types";
import { getMessages } from "../i18n";
import { workspaceMessages, activityLabel } from "../i18n/workspace";
import type { WorkspaceProps } from "../App";

export function ActivityPage({ locale, onError }: WorkspaceProps) {
  const t = getMessages(locale), w = workspaceMessages(locale);
  const [events, setEvents] = useState<AuditEvent[] | null>(null);
  const [loading, setLoading] = useState(true), [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setFailed(false);
    activityCommands.events().then((items) => {
      if (active) setEvents(items);
    }).catch((error) => {
      if (active) {
        if ((error as { code?: string })?.code === "vault_locked") onError(error);
        else setFailed(true);
      }
    }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [attempt, onError]);

  return <section aria-busy={loading}>
    <div className="page-heading"><div><h2>{t.pages.activity}</h2><p>{w.activityHint}</p></div></div>
    {loading ? <div className="quiet-state" role="status">{t.pages.loading}</div>
      : failed ? <div className="workspace-feedback"><p role="alert">{w.activityLoadError}</p><button className="button button-subtle" onClick={() => setAttempt(value => value + 1)}>{w.retry}</button></div>
      : !events?.length ? <div className="quiet-state">{t.pages.noEvents}</div>
      : <div className="event-list" role="list" aria-label={t.pages.activity}>{events.map((event) => <div className="event-row" role="listitem" key={event.id}>
        <span className="event-mark" aria-hidden="true">◇</span><strong className="workspace-wrap" title={event.event_type}>{activityLabel(locale, event.event_type)}</strong><span className="flex-1" /><time className="workspace-wrap" dateTime={event.created_at}>{new Date(event.created_at).toLocaleString(locale)}</time>
      </div>)}</div>}
  </section>;
}
