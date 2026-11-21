import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings, type WorkspaceProps } from "../App";
import { getMessages } from "../i18n";
import { workspaceMessages } from "../i18n/workspace";
import type { AuditEvent } from "../lib/types";
import { ActivityPage } from "./ActivityPage";
import { CategoriesPage } from "./CategoriesPage";
import { DashboardPage } from "./DashboardPage";
import { SecurityPage } from "./SecurityPage";

const api = vi.hoisted(() => ({ events: vi.fn(), dashboard: vi.fn(), save: vi.fn(), remove: vi.fn(), audit: vi.fn() }));
vi.mock("../lib/tauri", () => ({
  activityCommands: { events: api.events, dashboard: api.dashboard },
  categoryCommands: { save: api.save, remove: api.remove },
}));
vi.mock("../lib/backup", () => ({ backupCommands: { audit: api.audit } }));

const t = getMessages("en"), w = workspaceMessages("en");
const event: AuditEvent = { id: 1, event_type: "vault_unlocked", account_id: null, created_at: "2026-01-01T00:00:00Z" };
const audit = {
  checked_at: "2026-01-01T00:00:00Z", locked: false, integrity: { ok: true, user_version: 1 },
  corrupt_record_count: 0, duplicate_count: 0, invalid_config_count: 0, plaintext_file_findings: [],
  permission_findings: "", backup_count: 2, last_backup_at: "1767225600", backup_configured: true, time_status: "system",
};

function workspace(overrides: Partial<WorkspaceProps> = {}): WorkspaceProps {
  return {
    locale: "en", accounts: [], categories: [], codes: [], settings: defaultSettings,
    refresh: vi.fn(async () => {}), notify: vi.fn(), onError: vi.fn(), onAdd: vi.fn(), onEdit: vi.fn(),
    navigate: vi.fn(), saveSettings: vi.fn(async () => {}), ...overrides,
  };
}

beforeEach(() => vi.resetAllMocks());

describe("activity states", () => {
  it("shows loading instead of the empty state while events are pending", () => {
    api.events.mockReturnValue(new Promise(() => {}));
    render(<ActivityPage {...workspace()} />);
    expect(screen.getByText(t.pages.loading)).toBeVisible();
    expect(screen.queryByText(t.pages.noEvents)).not.toBeInTheDocument();
  });

  it("surfaces a load failure with a local retry that recovers the list", async () => {
    api.events.mockRejectedValueOnce({ code: "backend_unavailable" }).mockResolvedValueOnce([event]);
    const props = workspace();
    render(<ActivityPage {...props} />);
    expect(await screen.findByText(w.activityLoadError)).toBeVisible();
    expect(props.onError).not.toHaveBeenCalled();
    expect(screen.queryByText(t.pages.noEvents)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: w.retry }));
    expect(await screen.findByText("vault_unlocked")).toBeVisible();
    expect(api.events).toHaveBeenCalledTimes(2);
  });

  it("localizes known audit codes and keeps the raw code as the row title", async () => {
    api.events.mockResolvedValue([
      { id: 2, event_type: "code_copied", account_id: "a1", created_at: "2026-01-01T00:00:00Z" },
      event,
    ]);
    render(<ActivityPage {...workspace({ locale: "zh-CN" })} />);

    const copied = await screen.findByText("已复制验证码");
    expect(copied).toHaveAttribute("title", "code_copied");
    // An event type this build does not know is shown verbatim rather than hidden.
    expect(screen.getByText("vault_unlocked")).toHaveAttribute("title", "vault_unlocked");
  });
});

describe("category form", () => {
  const categories = [{ id: "work", name: "Work", color: "#111111" }];

  it("uses category wording and per-row accessible names", () => {
    render(<CategoriesPage {...workspace({ categories })} />);
    expect(screen.getByText(w.categoryColor)).toBeVisible();
    expect(screen.getByRole("button", { name: w.categoryAdd })).toBeVisible();
    expect(screen.getByRole("button", { name: `${w.categoryEdit}: Work` })).toBeVisible();
    expect(screen.getByRole("button", { name: `${w.categoryDelete}: Work` })).toBeVisible();
  });

  it("blocks duplicate submits and edits while saving keeps the draft", async () => {
    let finish!: (value: unknown) => void;
    api.save.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const props = workspace();
    render(<CategoriesPage {...props} />);
    const input = screen.getByRole("textbox", { name: w.categoryName });
    fireEvent.change(input, { target: { value: "Work" } });
    const submit = screen.getByRole("button", { name: w.categoryAdd });
    fireEvent.click(submit);
    fireEvent.click(submit);
    expect(api.save).toHaveBeenCalledExactlyOnceWith(null, "Work", "#7b73e8");
    expect(submit).toBeDisabled();
    expect(input).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent(w.saving);
    await act(async () => { finish({ id: "work", name: "Work", color: "#7b73e8" }); });
    await waitFor(() => expect(input).toBeEnabled());
    expect(input).toHaveValue("");
    expect(props.refresh).toHaveBeenCalledOnce();
  });

  it("reports a saved change whose refresh failed and retries only the refresh", async () => {
    api.save.mockResolvedValue({ id: "work", name: "Work", color: "#7b73e8" });
    const refresh = vi.fn().mockRejectedValueOnce(new Error("offline")).mockResolvedValue(undefined);
    render(<CategoriesPage {...workspace({ refresh })} />);
    fireEvent.change(screen.getByRole("textbox", { name: w.categoryName }), { target: { value: "Work" } });
    fireEvent.click(screen.getByRole("button", { name: w.categoryAdd }));
    expect(await screen.findByText(w.categoryRefreshError)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: w.retry }));
    await waitFor(() => expect(screen.queryByText(w.categoryRefreshError)).not.toBeInTheDocument());
    expect(refresh).toHaveBeenCalledTimes(2);
    expect(api.save).toHaveBeenCalledOnce();
  });
});

describe("security audit states", () => {
  it("distinguishes a failed first load from a genuine not-checked state", async () => {
    api.audit.mockRejectedValueOnce({ code: "backend_unavailable" }).mockResolvedValueOnce(audit);
    const props = workspace();
    render(<SecurityPage {...props} />);
    expect(await screen.findByText(w.auditLoadError)).toBeVisible();
    expect(screen.queryByText(t.status.notChecked)).not.toBeInTheDocument();
    expect(screen.getAllByText(w.unavailable).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: w.retry }));
    expect(await screen.findByText(t.status.ok)).toBeVisible();
    expect(screen.queryByText(w.auditLoadError)).not.toBeInTheDocument();
  });

  it("keeps the last successful report when a refresh fails", async () => {
    api.audit.mockResolvedValueOnce(audit).mockRejectedValueOnce(new Error("offline"));
    render(<SecurityPage {...workspace()} />);
    expect(await screen.findByText(t.status.ok)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: t.actions.refresh }));
    expect(await screen.findByText(w.auditRefreshError)).toBeVisible();
    expect(screen.getByText(t.status.ok)).toBeVisible();
    expect(screen.getByText(new RegExp(w.lastSuccessfulCheck))).toBeVisible();
  });
});

describe("dashboard statistics", () => {
  it("shows unknown copy counts as unavailable instead of zero and recovers on retry", async () => {
    api.dashboard.mockRejectedValueOnce({ code: "backend_unavailable" }).mockResolvedValueOnce({
      account_count: 0, category_count: 0, favorite_count: 0, copies_today: 3, recent_accounts: [],
    });
    render(<DashboardPage {...workspace()} />);
    expect(await screen.findByText(w.dashboardLoadError)).toBeVisible();
    expect(screen.getByText("—")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: w.retry }));
    expect(await screen.findByText("3")).toBeVisible();
    expect(screen.queryByText("—")).not.toBeInTheDocument();
  });
});