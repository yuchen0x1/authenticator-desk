import { useEffect, useRef, useState, type FormEvent } from "react";
import { categoryCommands } from "../lib/tauri";
import { getMessages } from "../i18n";
import { workspaceMessages } from "../i18n/workspace";
import type { WorkspaceProps } from "../App";

export function CategoriesPage({ locale, categories, refresh, onError }: WorkspaceProps) {
  const t = getMessages(locale), w = workspaceMessages(locale);
  const [name, setName] = useState("");
  const [color, setColor] = useState("#7b73e8");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState<"save" | "delete" | "refresh" | null>(null);
  const [failure, setFailure] = useState<"save" | "delete" | "refresh" | null>(null);
  const pending = useRef(false), live = useRef(true);

  useEffect(() => {
    live.current = true;
    return () => { live.current = false; };
  }, []);

  function resetForm() {
    setEditing(null);
    setName("");
  }

  async function run(kind: "save" | "delete" | "refresh", mutate?: () => Promise<unknown>) {
    if (pending.current) return;
    pending.current = true;
    setBusy(kind);
    setFailure(null);
    let step = kind;
    try {
      if (mutate) await mutate();
      step = "refresh";
      await refresh();
    } catch (error) {
      if (live.current) {
        if ((error as { code?: string })?.code === "vault_locked") onError(error);
        else setFailure(step);
      }
    } finally {
      pending.current = false;
      if (live.current) setBusy(null);
    }
  }

  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed || failure === "refresh") return;
    void run("save", async () => {
      await categoryCommands.save(editing, trimmed, color);
      if (live.current) resetForm();
    });
  }

  function remove(id: string, category: string) {
    if (pending.current || failure === "refresh" || !window.confirm(`${w.categoryDelete}: ${category}`)) return;
    void run("delete", async () => {
      await categoryCommands.remove(id);
      if (live.current && editing === id) resetForm();
    });
  }

  const blocked = busy !== null || failure === "refresh";
  return <section>
    <div className="page-heading"><div><h2>{t.pages.categories}</h2><p>{w.categoriesHint}</p></div></div>
    <form onSubmit={save} aria-busy={busy !== null}>
      <fieldset className="category-create" disabled={blocked}>
        <label className="min-w-0 flex-1">{w.categoryName}<input value={name} onChange={(event) => setName(event.target.value)} maxLength={64} required /></label>
        <label className="color-field">{w.categoryColor}<input type="color" value={color} onChange={(event) => setColor(event.target.value)} /></label>
        <button type="submit" className="button button-primary" disabled={!name.trim()}>{busy === "save" ? w.saving : editing ? t.actions.save : w.categoryAdd}</button>
        {editing && <button className="button button-subtle" type="button" onClick={() => { resetForm(); setFailure(null); }}>{t.actions.cancel}</button>}
      </fieldset>
    </form>
    {busy && <p className="workspace-feedback" role="status">{busy === "save" ? w.saving : w.working}</p>}
    {failure && <div className="workspace-feedback"><p role="alert">{failure === "refresh" ? w.categoryRefreshError : failure === "save" ? w.categorySaveError : w.categoryDeleteError}</p>
      {failure === "refresh" && <button className="button button-subtle" onClick={() => void run("refresh")}>{w.retry}</button>}
    </div>}
    {categories.length === 0 ? <div className="quiet-state">{w.categoriesEmpty}</div>
      : <div className="category-list" role="list" aria-label={t.pages.categories}>{categories.map((item) => <div className="category-row" role="listitem" key={item.id}>
        <span className="category-swatch shrink-0" aria-hidden="true" style={{ background: item.color }} /><strong className="workspace-wrap flex-1">{item.name}</strong>
        <button className="icon-button" disabled={blocked} aria-label={`${w.categoryEdit}: ${item.name}`} onClick={() => { setEditing(item.id); setName(item.name); setColor(item.color); setFailure(null); }}>✎</button>
        <button className="icon-button" disabled={blocked} aria-label={`${w.categoryDelete}: ${item.name}`} onClick={() => remove(item.id, item.name)}>×</button>
      </div>)}</div>}
  </section>;
}
