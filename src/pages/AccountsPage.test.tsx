import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceProps } from "../App";
import { getMessages } from "../i18n";
import { accountUiMessages } from "../i18n/accountUi";
import type { AccountView, CodeView } from "../lib/types";
import { AccountsPage } from "./AccountsPage";

const api = vi.hoisted(() => ({ move: vi.fn(), remove: vi.fn(), reorder: vi.fn(), favorite: vi.fn(), advanceHotp: vi.fn(), copy: vi.fn() }));
vi.mock("../lib/tauri", () => ({ accountCommands: api }));

const t = getMessages("en");
const actionName = (action: string, account: AccountView) => `${action}: ${account.issuer} · ${account.account_name}`;
const alpha: AccountView = {
  id: "alpha", issuer: "Alpha", account_name: "test-alpha", category_id: "work", tags: ["work"],
  favorite: true, algorithm: "SHA1", digits: 6, kind: "totp", period_secs: 30, t0: "0",
  hotp_counter: "0", notes_present: false, sort_order: 0, created_at: "", updated_at: "",
};
const beta: AccountView = {
  ...alpha, id: "beta", issuer: "Beta", account_name: "test-beta", category_id: null, tags: [], favorite: false,
};

function workspace(): WorkspaceProps {
  return {
    locale: "en", accounts: [alpha, beta], categories: [{ id: "work", name: "Work", color: "#111111" }], codes: [],
    settings: {
      locale: "en", theme: "dark", idle_timeout_secs: 900, clipboard_ttl_secs: 30, hide_codes: false,
      view_mode: "cards", animations: false, compact: false, font_size: 14, default_algorithm: "SHA1",
      default_digits: 6, default_period_secs: 30, close_to_tray: false, minimize_to_tray: false,
      startup: false, backup_enabled: false, backup_interval_hours: 24, backup_retention: 7, show_next_code: false,
    },
    refresh: vi.fn(async () => {}), notify: vi.fn(), onError: vi.fn(), onAdd: vi.fn(), onEdit: vi.fn(),
    navigate: vi.fn(), saveSettings: vi.fn(async () => {}),
  };
}

beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.restoreAllMocks());

function selectBoth() {
  fireEvent.click(screen.getByRole("checkbox", { name: actionName(t.status.selected, alpha) }));
  fireEvent.click(screen.getByRole("checkbox", { name: actionName(t.status.selected, beta) }));
  expect(screen.getByText(`2 ${t.status.selected}`)).toBeVisible();
}

describe("bulk selection after filtering", () => {
  it("deletes only still-visible selected accounts", async () => {
    const props = workspace();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<AccountsPage {...props} mode="accounts" />);
    selectBoth();

    fireEvent.change(screen.getByRole("textbox", { name: t.fields.search }), { target: { value: "alpha" } });
    expect(screen.getByText(`1 ${t.status.selected}`)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: t.actions.deleteSelected }));

    expect(confirm).toHaveBeenCalledWith(`${t.actions.deleteSelected}: 1`);
    expect(api.remove).toHaveBeenCalledExactlyOnceWith([alpha.id]);
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });

  it("moves only still-visible selected accounts", async () => {
    const props = workspace();
    render(<AccountsPage {...props} mode="accounts" />);
    selectBoth();

    fireEvent.change(screen.getByRole("combobox", { name: t.fields.category }), { target: { value: "work" } });
    expect(screen.getByText(`1 ${t.status.selected}`)).toBeVisible();
    fireEvent.change(screen.getByRole("combobox", { name: t.actions.move }), { target: { value: "work" } });

    expect(api.move).toHaveBeenCalledExactlyOnceWith([alpha.id], "work");
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });

  it("unselects accounts hidden by a filter, even after they become visible again", () => {
    render(<AccountsPage {...workspace()} mode="accounts" />);
    selectBoth();

    const search = screen.getByRole("textbox", { name: t.fields.search });
    fireEvent.change(search, { target: { value: "no matching accounts" } });
    expect(screen.queryByRole("button", { name: t.actions.deleteSelected })).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: t.actions.move })).not.toBeInTheDocument();

    fireEvent.change(search, { target: { value: "" } });
    expect(screen.getByRole("checkbox", { name: actionName(t.status.selected, alpha) })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: actionName(t.status.selected, beta) })).not.toBeChecked();
    expect(api.remove).not.toHaveBeenCalled();
    expect(api.move).not.toHaveBeenCalled();
  });

  it("keeps selection and resets the destination so the same move can be retried", async () => {
    const props = workspace();
    const error = new Error("move failed");
    api.move.mockRejectedValueOnce(error).mockResolvedValueOnce(undefined);
    render(<AccountsPage {...props} mode="accounts" />);
    fireEvent.click(screen.getByRole("checkbox", { name: actionName(t.status.selected, beta) }));
    const move = screen.getByRole("combobox", { name: t.actions.move });
    fireEvent.change(move, { target: { value: "work" } });
    expect(move).toBeDisabled();
    await waitFor(() => expect(move).toBeEnabled());
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(error);
    expect(move).toHaveValue("");
    expect(screen.getByRole("checkbox", { name: actionName(t.status.selected, beta) })).toBeChecked();
    expect(props.refresh).not.toHaveBeenCalled();
    fireEvent.change(move, { target: { value: "work" } });
    expect(api.move).toHaveBeenCalledTimes(2);
    expect(api.move).toHaveBeenLastCalledWith([beta.id], "work");
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });
});

describe("account filters and order", () => {
  it("offers Clear filters on no match and resets search, category and tag", () => {
    render(<AccountsPage {...workspace()} mode="accounts" />);
    const search = screen.getByRole("textbox", { name: t.fields.search });
    const category = screen.getByRole("combobox", { name: t.fields.category });
    const tags = screen.getByRole("combobox", { name: t.fields.tags });
    fireEvent.change(category, { target: { value: "work" } });
    fireEvent.change(tags, { target: { value: "work" } });
    fireEvent.change(search, { target: { value: "beta" } });
    expect(screen.getByRole("heading", { name: t.pages.noMatch })).toBeVisible();
    expect(screen.queryByRole("heading", { name: t.pages.empty })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: t.actions.clearFilters }));
    expect(search).toHaveValue("");
    expect(category).toHaveValue("");
    expect(tags).toHaveValue("");
    expect(within(screen.getByRole("list", { name: t.pages.accounts })).getAllByRole("listitem")).toHaveLength(2);
  });

  it("reports a genuinely empty account scope even with search text", () => {
    const props = workspace();
    render(<AccountsPage {...props} accounts={[]} mode="accounts" />);
    fireEvent.change(screen.getByRole("textbox", { name: t.fields.search }), { target: { value: "alpha" } });
    expect(screen.getByRole("heading", { name: t.pages.empty })).toBeVisible();
    expect(screen.queryByRole("heading", { name: t.pages.noMatch })).not.toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: t.actions.add }).at(-1)!);
    expect(props.onAdd).toHaveBeenCalledOnce();
  });

  it("renders a named row for each account and disables boundary moves", async () => {
    const props = workspace();
    render(<AccountsPage {...props} mode="accounts" />);
    const order = screen.getByRole("region", { name: accountUiMessages("en").orderTitle });
    const [first, last] = within(order).getAllByRole("listitem");
    expect(within(first).getByText("Alpha · test-alpha")).toBeVisible();
    expect(within(last).getByText("Beta · test-beta")).toBeVisible();
    const firstUp = within(first).getByRole("button", { name: actionName(t.actions.moveUp, alpha) });
    const lastDown = within(last).getByRole("button", { name: actionName(t.actions.moveDown, beta) });
    expect(firstUp).toBeDisabled();
    expect(lastDown).toBeDisabled();
    fireEvent.click(firstUp);
    fireEvent.click(lastDown);
    expect(api.reorder).not.toHaveBeenCalled();
    fireEvent.click(within(first).getByRole("button", { name: actionName(t.actions.moveDown, alpha) }));
    expect(api.reorder).toHaveBeenCalledExactlyOnceWith([beta.id, alpha.id]);
    await waitFor(() => expect(props.refresh).toHaveBeenCalledOnce());
  });

  it.each(["search", "category", "tag", "favorites"] as const)("removes reorder actions while scoped by %s", (scope) => {
    const props = workspace();
    const favored = { ...beta, id: "favored", account_name: "test-favorite", favorite: true };
    render(<AccountsPage {...props} accounts={[alpha, beta, favored]} mode={scope === "favorites" ? "favorites" : "accounts"} />);
    if (scope === "search") fireEvent.change(screen.getByRole("textbox", { name: t.fields.search }), { target: { value: "test-" } });
    if (scope === "category") fireEvent.change(screen.getByRole("combobox", { name: t.fields.category }), { target: { value: "work" } });
    if (scope === "tag") fireEvent.change(screen.getByRole("combobox", { name: t.fields.tags }), { target: { value: "work" } });
    const order = screen.getByRole("region", { name: accountUiMessages("en").orderTitle });
    expect(within(order).queryByRole("button")).not.toBeInTheDocument();
    expect(within(order).getByText(scope === "favorites" ? accountUiMessages("en").reorderFavorites : t.actions.reorderFiltered)).toBeVisible();
    if (scope === "favorites") expect(within(screen.getByRole("list", { name: t.pages.favorites })).getAllByRole("listitem")).toHaveLength(2);
    expect(api.reorder).not.toHaveBeenCalled();
  });

  it("reports a refresh rejection through onError", async () => {
    const props = workspace();
    const error = new Error("refresh failed");
    const refresh = vi.fn().mockRejectedValue(error);
    render(<AccountsPage {...props} refresh={refresh} mode="accounts" />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.refresh }));
    await waitFor(() => expect(props.onError).toHaveBeenCalledExactlyOnceWith(error));
    expect(refresh).toHaveBeenCalledOnce();
  });
});

describe("account mutation pending state", () => {
  it("guards delete before confirmation, and disables the batch actions until failure", async () => {
    const props = workspace();
    const request = deferred<void>();
    const error = new Error("delete failed");
    api.remove.mockReturnValue(request.promise);
    render(<AccountsPage {...props} mode="accounts" />);
    selectBoth();
    const remove = screen.getByRole("button", { name: t.actions.deleteSelected });
    const move = screen.getByRole("combobox", { name: t.actions.move });
    const confirm = vi.spyOn(window, "confirm").mockImplementationOnce(() => {
      fireEvent.click(remove);
      return true;
    });
    fireEvent.click(remove);
    expect(confirm).toHaveBeenCalledExactlyOnceWith(`${t.actions.deleteSelected}: 2`);
    expect(api.remove).toHaveBeenCalledExactlyOnceWith([alpha.id, beta.id]);
    expect(remove).toBeDisabled();
    expect(move).toBeDisabled();
    await act(async () => request.reject(error));
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(error);
    expect(remove).toBeEnabled();
    expect(move).toBeEnabled();
  });

  it("holds all reorder actions while a write and its refresh are pending", async () => {
    const props = workspace();
    const write = deferred<void>();
    const view = deferred<void>();
    const refresh = vi.fn(() => view.promise);
    render(<AccountsPage {...props} refresh={refresh} mode="accounts" />);
    const up = screen.getByRole("button", { name: actionName(t.actions.moveUp, beta) });
    const down = screen.getByRole("button", { name: actionName(t.actions.moveDown, alpha) });
    api.reorder.mockImplementationOnce(() => {
      fireEvent.click(up);
      return write.promise;
    });
    fireEvent.click(down);
    expect(api.reorder).toHaveBeenCalledExactlyOnceWith([beta.id, alpha.id]);
    expect(up).toBeDisabled();
    expect(down).toBeDisabled();
    await act(async () => write.resolve());
    expect(refresh).toHaveBeenCalledOnce();
    expect(up).toBeDisabled();
    await act(async () => view.resolve());
    expect(up).toBeEnabled();
    expect(down).toBeEnabled();
  });

  it("guards favorite per account and leaves other accounts actionable", async () => {
    const props = workspace();
    const request = deferred<void>();
    render(<AccountsPage {...props} mode="accounts" />);
    const favorite = screen.getByRole("button", { name: actionName(t.actions.favorite, beta) });
    const unfavorite = screen.getByRole("button", { name: actionName(t.actions.unfavorite, alpha) });
    api.favorite.mockImplementationOnce(() => {
      fireEvent.click(favorite);
      return request.promise;
    });
    fireEvent.click(favorite);
    expect(api.favorite).toHaveBeenCalledExactlyOnceWith(beta.id, true);
    expect(favorite).toBeDisabled();
    expect(unfavorite).toBeEnabled();
    await act(async () => request.resolve());
    expect(favorite).toBeEnabled();
  });

  it("guards hide settings synchronously and masks code actions while saving", async () => {
    const props = workspace();
    const request = deferred<void>();
    const error = new Error("settings failed");
    let hide!: HTMLElement;
    let advance!: HTMLElement;
    let copy!: HTMLElement;
    const saveSettings = vi.fn(() => {
      fireEvent.click(hide);
      fireEvent.click(advance);
      fireEvent.click(copy);
      return request.promise;
    });
    render(<AccountsPage {...props} saveSettings={saveSettings} accounts={[hotp]} codes={[hotpCode()]} mode="accounts" />);
    hide = screen.getByRole("button", { name: t.settings.hideCodes });
    advance = screen.getByRole("button", { name: actionName(t.actions.advanceHotp, hotp) });
    copy = screen.getByRole("button", { name: actionName(t.actions.copy, hotp) });
    fireEvent.click(hide);
    expect(saveSettings).toHaveBeenCalledExactlyOnceWith({ ...props.settings, hide_codes: true });
    expect(hide).toBeDisabled();
    expect(hide).toHaveAttribute("aria-busy", "true");
    expect(advance).toBeDisabled();
    expect(copy).toBeDisabled();
    expect(api.advanceHotp).not.toHaveBeenCalled();
    expect(api.copy).not.toHaveBeenCalled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    await act(async () => request.reject(error));
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(error);
    expect(hide).toBeEnabled();
  });

  it("coalesces manual refresh clicks until the request settles", async () => {
    const props = workspace();
    const request = deferred<void>();
    let button!: HTMLElement;
    const refresh = vi.fn(() => {
      fireEvent.click(button);
      return request.promise;
    });
    render(<AccountsPage {...props} refresh={refresh} mode="accounts" />);
    button = screen.getByRole("button", { name: t.actions.refresh });
    fireEvent.click(button);
    expect(refresh).toHaveBeenCalledOnce();
    expect(button).toBeDisabled();
    await act(async () => request.resolve());
    expect(button).toBeEnabled();
  });
});

const hotp: AccountView = { ...alpha, kind: "hotp" };
function hotpCode(value = "123456", generatedAt = Math.floor(Date.now() / 1000)): CodeView {
  return { account_id: hotp.id, code: value, generated_at: generatedAt, remaining_secs: 0, progress: 0, clock_offset_secs: 0, wall_clock_jump: false };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

describe("HOTP advance safety", () => {
  it("blocks reentry and Copy synchronously before React commits the pending state", async () => {
    const props = workspace();
    const request = deferred<CodeView>();
    let advance!: HTMLElement;
    let copy!: HTMLElement;
    api.advanceHotp.mockImplementationOnce(() => {
      fireEvent.click(advance);
      fireEvent.click(copy);
      return request.promise;
    });
    const old = hotpCode();
    render(<AccountsPage {...props} accounts={[hotp]} codes={[old]} mode="accounts" />);
    advance = screen.getByRole("button", { name: actionName(t.actions.advanceHotp, hotp) });
    copy = screen.getByRole("button", { name: actionName(t.actions.copy, hotp) });
    fireEvent.click(advance);
    expect(api.advanceHotp).toHaveBeenCalledExactlyOnceWith(hotp.id);
    expect(api.copy).not.toHaveBeenCalled();
    expect(advance).toBeDisabled();
    expect(copy).toBeDisabled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    await act(async () => request.resolve(hotpCode("654321")));
    expect(advance).toBeEnabled();
    expect(copy).toBeDisabled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
  });

  it("keeps old and same-second polls invalid until a post-advance code snapshot", async () => {
    const props = workspace();
    const request = deferred<CodeView>();
    const refresh = deferred<void>();
    const now = 1795132859;
    vi.spyOn(Date, "now").mockReturnValue(now * 1000);
    const old = hotpCode("123456", now);
    api.advanceHotp.mockReturnValue(request.promise);
    const refreshFn = vi.fn(() => refresh.promise);
    const { rerender } = render(<AccountsPage {...props} accounts={[hotp]} codes={[old]} refresh={refreshFn} mode="accounts" />);
    const advance = screen.getByRole("button", { name: actionName(t.actions.advanceHotp, hotp) });
    const copy = screen.getByRole("button", { name: actionName(t.actions.copy, hotp) });
    fireEvent.click(advance);
    await act(async () => request.resolve(hotpCode("654321", now)));
    expect(advance).toBeDisabled();
    expect(copy).toBeDisabled();
    rerender(<AccountsPage {...props} accounts={[hotp]} codes={[{ ...old }]} refresh={refreshFn} mode="accounts" />);
    await act(async () => refresh.resolve());
    expect(advance).toBeEnabled();
    expect(copy).toBeDisabled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    // The poll is fresh in sample time, not merely a new object or digits.
    const fresh = { ...hotpCode("654321", now - 3600 + 1), clock_offset_secs: 3600 };
    rerender(<AccountsPage {...props} accounts={[hotp]} codes={[fresh]} refresh={refreshFn} mode="accounts" />);
    expect(screen.getByText("654 321")).toBeVisible();
    expect(copy).toBeEnabled();
    fireEvent.click(copy);
    await waitFor(() => expect(api.copy).toHaveBeenCalledExactlyOnceWith(hotp.id, "654321"));
    expect(props.notify).toHaveBeenCalledWith(t.status.copied);
  });

  it("does not restore the old code after a failed advance or reveal the refreshed code while hidden", async () => {
    const props = workspace();
    const error = new Error("advance failed");
    api.advanceHotp.mockRejectedValue(error);
    const old = hotpCode();
    const { rerender } = render(<AccountsPage {...props} accounts={[hotp]} codes={[old]} mode="accounts" />);
    const advance = screen.getByRole("button", { name: actionName(t.actions.advanceHotp, hotp) });
    const copy = screen.getByRole("button", { name: actionName(t.actions.copy, hotp) });
    fireEvent.click(advance);
    await waitFor(() => expect(props.onError).toHaveBeenCalledExactlyOnceWith(error));
    expect(advance).toBeEnabled();
    expect(copy).toBeDisabled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    const fresh = hotpCode("654321", Math.floor(Date.now() / 1000) + 1);
    rerender(<AccountsPage {...props} settings={{ ...props.settings, hide_codes: true }} accounts={[hotp]} codes={[fresh]} mode="accounts" />);
    expect(advance).toBeDisabled();
    expect(copy).toBeDisabled();
    expect(screen.queryByText("654 321")).not.toBeInTheDocument();
    fireEvent.click(advance);
    fireEvent.click(copy);
    expect(api.advanceHotp).toHaveBeenCalledOnce();
    expect(api.copy).not.toHaveBeenCalled();
    rerender(<AccountsPage {...props} accounts={[hotp]} codes={[]} mode="accounts" />);
    expect(copy).toBeDisabled();
    expect(screen.queryByText("654 321")).not.toBeInTheDocument();
  });

  it("keeps the stale code invalid and permits a deliberate retry after refresh fails", async () => {
    const props = workspace();
    const error = new Error("metadata refresh failed");
    const refresh = vi.fn().mockRejectedValueOnce(error).mockResolvedValue(undefined);
    api.advanceHotp.mockResolvedValue(hotpCode("654321"));
    render(<AccountsPage {...props} accounts={[hotp]} codes={[hotpCode()]} refresh={refresh} mode="accounts" />);
    const advance = screen.getByRole("button", { name: actionName(t.actions.advanceHotp, hotp) });
    fireEvent.click(advance);
    await waitFor(() => expect(props.onError).toHaveBeenCalledExactlyOnceWith(error));
    expect(advance).toBeEnabled();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: actionName(t.actions.copy, hotp) })).toBeDisabled();
    fireEvent.click(advance);
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(2));
    expect(api.advanceHotp).toHaveBeenCalledTimes(2);
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
  });
});
