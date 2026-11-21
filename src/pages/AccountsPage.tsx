import { useEffect, useMemo, useRef, useState } from "react";
import { accountCommands } from "../lib/tauri";
import type { AccountView, CodeView } from "../lib/types";
import { getMessages } from "../i18n";
import { accountUiMessages } from "../i18n/accountUi";
import type { WorkspaceProps } from "../App";
import type { Destination } from "../components/layout/AppShell";
import { AccountList } from "../components/accounts/AccountList";
import { EmptyState } from "../components/common/EmptyState";

type HotpFence = { pending: true } | { pending: false; settledAt: number };
type ActionKey = "batch" | "reorder" | "refresh" | "hide" | `favorite:${string}`;

function isCurrentHotpCode(code: CodeView, fence?: HotpFence) {
  // Use the wall-clock sample time, undoing the backend OTP clock offset.
  // Reject the whole settling second: a poll already in flight could have
  // sampled the old counter in that same second.
  return !fence || (!fence.pending && code.generated_at + code.clock_offset_secs > fence.settledAt);
}

export function AccountsPage({ locale, accounts, categories, codes, settings, refresh, notify, onError, onAdd, onEdit, saveSettings, mode }: WorkspaceProps & { mode: Destination }) {
  const t = getMessages(locale);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("");
  const [tag, setTag] = useState("");
  const [moveTarget, setMoveTarget] = useState("");
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [pending, setPending] = useState(() => new Set<ActionKey>());
  const pendingRef = useRef(pending);
  const live = useRef(true);
  const [hotpFences, setHotpFences] = useState(() => new Map<string, HotpFence>());
  const hotpRef = useRef(hotpFences);
  const scoped = useMemo(() => mode === "favorites" ? accounts.filter((account) => account.favorite) : accounts, [accounts, mode]);
  const filtered = useMemo(() => scoped.filter((account) =>
    (!search || `${account.issuer} ${account.account_name}`.toLocaleLowerCase().includes(search.toLocaleLowerCase())) &&
    (!category || account.category_id === category) &&
    (!tag || account.tags.includes(tag)),
  ), [scoped, category, search, tag]);
  const visibleIds = useMemo(() => new Set(filtered.map((account) => account.id)), [filtered]);
  const visibleSelected = new Set([...selected].filter((id) => visibleIds.has(id)));
  const tags = [...new Set(accounts.flatMap((account) => account.tags))].sort();
  const filtersActive = Boolean(search || category || tag);
  const canReorder = !filtersActive && mode !== "favorites";
  const advancing = new Set([...hotpFences].filter(([, fence]) => fence.pending).map(([id]) => id));
  // Only display parent-supplied codes, so hide/lock and App's code clearing
  // still apply even when the advance IPC has already returned a new value.
  const availableCodes = codes.filter((code) => isCurrentHotpCode(code, hotpFences.get(code.account_id)));

  useEffect(() => {
    live.current = true;
    return () => { live.current = false; };
  }, []);

  // Prune selection when filtering or refreshing so hidden accounts cannot
  // silently become selected again when they return to the list.
  useEffect(() => {
    setSelected((previous) => {
      const next = new Set([...previous].filter((id) => visibleIds.has(id)));
      return next.size === previous.size ? previous : next;
    });
  }, [visibleIds]);

  async function runAction(key: ActionKey, action: () => Promise<void>) {
    if (!live.current || pendingRef.current.has(key)) return;
    pendingRef.current = new Set(pendingRef.current).add(key);
    setPending(pendingRef.current);
    try { await action(); }
    catch (error) { if (live.current) onError(error); }
    finally {
      const next = new Set(pendingRef.current);
      next.delete(key);
      pendingRef.current = next;
      if (live.current) setPending(next);
    }
  }

  function favorite(account: AccountView) {
    return runAction(`favorite:${account.id}`, async () => {
      await accountCommands.favorite(account.id, !account.favorite);
      await refresh();
    });
  }

  function move(destination: string) {
    if (!visibleSelected.size) return;
    return runAction("batch", async () => {
      setMoveTarget(destination);
      try {
        await accountCommands.move([...visibleSelected], destination === "uncategorized" ? null : destination);
        setSelected(new Set());
        await refresh();
      } finally { setMoveTarget(""); }
    });
  }

  function remove() {
    if (!visibleSelected.size) return;
    return runAction("batch", async () => {
      if (!window.confirm(`${t.actions.deleteSelected}: ${visibleSelected.size}`)) return;
      await accountCommands.remove([...visibleSelected]);
      setSelected(new Set());
      await refresh();
      notify(t.status.saved);
    });
  }

  function reorder(account: AccountView, direction: -1 | 1) {
    if (!canReorder) return;
    const index = accounts.findIndex((item) => item.id === account.id);
    const next = [...accounts];
    const target = index + direction;
    if (index < 0 || target < 0 || target >= next.length) return;
    [next[index], next[target]] = [next[target], next[index]];
    return runAction("reorder", async () => {
      await accountCommands.reorder(next.map((item) => item.id));
      await refresh();
    });
  }

  function setHotpFence(id: string, fence: HotpFence) {
    const next = new Map(hotpRef.current).set(id, fence);
    hotpRef.current = next;
    setHotpFences(next);
  }

  // App's code polls are serial. After a post-advance sample arrives, later
  // snapshots no longer need to be compared to the advance's wall clock.
  useEffect(() => {
    const previous = hotpRef.current;
    let next: typeof previous | undefined;
    for (const code of codes) {
      const fence = previous.get(code.account_id);
      if (fence && isCurrentHotpCode(code, fence)) {
        next ??= new Map(previous);
        next.delete(code.account_id);
      }
    }
    if (next) {
      hotpRef.current = next;
      setHotpFences(next);
    }
  }, [codes, hotpFences]);

  async function advance(account: AccountView) {
    // Check/set synchronously: React may not have committed the disabled
    // button yet, and Copy must already reject the previous code.
    if (settings.hide_codes || pendingRef.current.has("hide") || account.kind !== "hotp" || hotpRef.current.get(account.id)?.pending) return;
    setHotpFence(account.id, { pending: true });
    let settledAt: number | undefined;
    try {
      await accountCommands.advanceHotp(account.id);
      settledAt = Math.floor(Date.now() / 1000);
      await refresh();
    } catch (error) { onError(error); }
    finally {
      // A failed IPC can also have changed the counter. Never restore the old
      // snapshot on either an advance error or a metadata refresh error.
      setHotpFence(account.id, { pending: false, settledAt: settledAt ?? Math.floor(Date.now() / 1000) });
    }
  }

  async function copy(account: AccountView) {
    const current = codes.find((item) => item.account_id === account.id);
    if (settings.hide_codes || pendingRef.current.has("hide") || !current || !isCurrentHotpCode(current, hotpRef.current.get(account.id)) || (account.kind === "totp" && Math.floor(Date.now() / 1000) - current.clock_offset_secs >= current.generated_at + current.remaining_secs)) return;
    try {
      await accountCommands.copy(account.id, current.code);
      notify(t.status.copied);
    } catch (error) { onError(error); }
  }

  function clearFilters() {
    setSearch("");
    setCategory("");
    setTag("");
  }

  const favoriting = new Set(accounts.filter((account) => pending.has(`favorite:${account.id}`)).map((account) => account.id));
  const batchBusy = pending.has("batch");
  const ui = accountUiMessages(locale);
  return <section>
    <div className="page-heading">
      <div><h2>{mode === "codes" ? t.pages.codes : mode === "favorites" ? t.pages.favorites : t.pages.accounts}</h2><p>{t.pages.accountsHint}</p></div>
      <div className="flex gap-2">
        <button className="button button-subtle" disabled={pending.has("hide")} aria-busy={pending.has("hide")} onClick={() => void runAction("hide", () => saveSettings({ ...settings, hide_codes: !settings.hide_codes }))} aria-label={t.settings.hideCodes} aria-pressed={settings.hide_codes}>{settings.hide_codes ? "◉" : "◌"}</button>
        <button className="button button-primary" onClick={onAdd}>＋ {t.actions.add}</button>
      </div>
    </div>
    <div className="toolbar">
      <label className="search-field"><span aria-hidden="true">⌕</span><input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t.fields.search} aria-label={t.fields.search} /></label>
      <select aria-label={t.fields.category} value={category} onChange={(event) => setCategory(event.target.value)}>
        <option value="">{t.fields.allCategories}</option>
        {categories.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </select>
      <select aria-label={t.fields.tags} value={tag} onChange={(event) => setTag(event.target.value)}>
        <option value="">{t.fields.allTags}</option>
        {tags.map((item) => <option key={item}>{item}</option>)}
      </select>
      {filtersActive && (filtered.length > 0 || scoped.length === 0) && <button className="button button-subtle" onClick={clearFilters}>{t.actions.clearFilters}</button>}
      <button className="button button-subtle" disabled={pending.has("refresh")} aria-busy={pending.has("refresh")} onClick={() => void runAction("refresh", refresh)} title={t.actions.refresh} aria-label={t.actions.refresh}>↻</button>
    </div>
    {visibleSelected.size > 0 && <div className="batch-bar" aria-busy={batchBusy}>
      <span>{visibleSelected.size} {t.status.selected}</span>
      <select aria-label={t.actions.move} value={moveTarget} disabled={batchBusy} onChange={(event) => {
        const destination = event.target.value;
        if (destination) void move(destination);
      }}>
        <option value="" disabled>{t.actions.move}</option>
        <option value="uncategorized">{t.fields.uncategorized}</option>
        {categories.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </select>
      <button className="button button-danger" disabled={batchBusy} onClick={() => void remove()}>{t.actions.deleteSelected}</button>
      <button className="button button-subtle" disabled={batchBusy} onClick={() => setSelected(new Set())}>{t.actions.cancel}</button>
    </div>}
    {filtered.length === 0
      ? filtersActive && scoped.length > 0
        ? <EmptyState title={t.pages.noMatch} detail={t.pages.noMatchHint} action={t.actions.clearFilters} onAction={clearFilters} />
        : <EmptyState title={t.pages.empty} detail={t.pages.emptyHint} action={t.actions.add} onAction={onAdd} />
      : <AccountList viewMode={settings.view_mode} listLabel={mode === "codes" ? t.pages.codes : mode === "favorites" ? t.pages.favorites : t.pages.accounts} accounts={filtered} codes={availableCodes} selected={visibleSelected} masked={settings.hide_codes || pending.has("hide")} copyAvailable advancing={advancing} favoriting={favoriting}
          labels={{ copy: t.actions.copy, edit: t.actions.edit, favorite: t.actions.favorite, unfavorite: t.actions.unfavorite, select: t.status.selected, hotp: t.fields.hotp, advanceHotp: t.actions.advanceHotp, hiddenCode: t.status.hiddenCode }}
          onSelect={(id, checked) => setSelected((previous) => {
            const next = new Set(previous);
            if (checked) next.add(id);
            else next.delete(id);
            return next;
          })}
          onEdit={onEdit} onFavorite={favorite} onCopy={copy} onAdvance={advance} />}
    {filtered.length > 0 && accounts.length > 1 && <section className="keyboard-order account-order" aria-label={ui.orderTitle}>
      <h3 className="account-order-heading">{ui.orderTitle}</h3>
      {canReorder ? <ul className="account-order-list">
        {filtered.map((account, index) => {
          const name = `${account.issuer} · ${account.account_name}`;
          return <li key={account.id} className="account-order-row">
            <span className="account-order-name" title={name}>{name}</span>
            <div className="account-order-actions" aria-busy={pending.has("reorder")}>
              <button className="button button-subtle" title={`${t.actions.moveUp}: ${name}`} aria-label={`${t.actions.moveUp}: ${name}`} disabled={pending.has("reorder") || index === 0} onClick={() => void reorder(account, -1)}><span aria-hidden="true">↑ </span>{t.actions.moveUp}</button>
              <button className="button button-subtle" title={`${t.actions.moveDown}: ${name}`} aria-label={`${t.actions.moveDown}: ${name}`} disabled={pending.has("reorder") || index === filtered.length - 1} onClick={() => void reorder(account, 1)}><span aria-hidden="true">↓ </span>{t.actions.moveDown}</button>
            </div>
          </li>;
        })}
      </ul> : <p className="account-order-hint">{mode === "favorites" ? ui.reorderFavorites : t.actions.reorderFiltered}</p>}
    </section>}
  </section>;
}
