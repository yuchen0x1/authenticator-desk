import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getMessages } from "../../i18n";
import { accountCommands } from "../../lib/tauri";
import type { AccountDetails, AccountView } from "../../lib/types";
import { AccountDialog } from "./AccountDialog";

vi.mock("../../lib/tauri", () => ({ accountCommands: { details: vi.fn() } }));

const messages = getMessages("en");
const t = {
  ...messages.fields, ...messages.actions, ...messages.status,
  required: messages.status.invalid, secretPrivacy: messages.pages.accountsHint,
  loading: messages.pages.loading, retry: messages.desktop.retry,
};
const account: AccountView = {
  id: "account-one", issuer: "Service", account_name: "test-user", algorithm: "SHA1", digits: 6,
  kind: "totp", period_secs: 30, t0: "0", hotp_counter: "0", category_id: null, tags: [],
  notes_present: true, favorite: false, sort_order: 0,
  created_at: "2026-11-17T00:00:00Z", updated_at: "2026-11-17T00:00:00Z",
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function props(): ComponentProps<typeof AccountDialog> {
  return { categories: [], defaults: { algorithm: "SHA1", digits: 6, period: 30 }, t, onClose: vi.fn(), onSave: vi.fn(async () => {}) };
}
function fillNewAccount() {
  fireEvent.change(screen.getByLabelText(t.issuer), { target: { value: "Service" } });
  fireEvent.change(screen.getByLabelText(t.accountName), { target: { value: "test-user" } });
  fireEvent.change(screen.getByLabelText(t.secret), { target: { value: "JBSWY3DPEHPK3PXP" } });
}

beforeEach(() => {
  vi.mocked(accountCommands.details).mockReset().mockResolvedValue({ account, notes: "Existing private notes" });
});

describe("AccountDialog", () => {
  it("uses flat loading/error/retry strings and keeps saving blocked until a single-flight details retry succeeds", async () => {
    const first = deferred<AccountDetails>(), retry = deferred<AccountDetails>();
    vi.mocked(accountCommands.details).mockReturnValueOnce(first.promise).mockReturnValueOnce(retry.promise);
    const handlers = props();
    const { container } = render(<AccountDialog {...handlers} account={account} />);
    const dialog = screen.getByRole("dialog", { name: t.edit });
    expect(container).not.toContainElement(dialog);
    expect(dialog.parentElement?.parentElement).toBe(document.body);
    expect(within(dialog).getByRole("status")).toHaveTextContent(t.loading);
    expect(screen.getByLabelText(t.issuer)).toBeDisabled();
    expect(screen.getByRole("button", { name: t.save })).toBeDisabled();
    const close = within(dialog).getAllByRole<HTMLButtonElement>("button", { name: t.cancel })[0];
    expect(close).toHaveFocus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(close).toHaveFocus();
    fireEvent.submit(dialog.querySelector("form")!);
    expect(handlers.onSave).not.toHaveBeenCalled();

    await act(async () => first.reject(new Error("private backend detail")));
    expect(screen.getByRole("alert")).toHaveTextContent(t.detailsLoadError);
    expect(screen.queryByText("private backend detail")).not.toBeInTheDocument();
    const retryButton = screen.getByRole("button", { name: t.retry });
    close.focus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(retryButton).toHaveFocus();
    act(() => { retryButton.click(); retryButton.click(); });
    expect(accountCommands.details).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByLabelText(t.notes)).toBeDisabled();
    expect(close).toHaveFocus();

    await act(async () => retry.resolve({ account, notes: "Retained after retry" }));
    expect(screen.getByLabelText(t.notes)).toHaveValue("Retained after retry");
    expect(screen.getByRole("button", { name: t.save })).toBeEnabled();
    await act(async () => fireEvent.submit(dialog.querySelector("form")!));
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ notes: "Retained after retry", secret: null }));
  });

  it("blocks synchronous duplicate submits and dismissal, clears the secret, and exposes a local retryable save error", async () => {
    const pending = deferred<void>();
    const handlers = props();
    vi.mocked(handlers.onSave).mockReturnValueOnce(pending.promise);
    render(<AccountDialog {...handlers} />);
    fillNewAccount();
    const dialog = screen.getByRole("dialog");
    const form = dialog.querySelector("form")!;
    act(() => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      fireEvent.keyDown(dialog, { key: "Escape" });
      fireEvent.mouseDown(dialog.parentElement!);
    });
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(handlers.onClose).not.toHaveBeenCalled();
    expect(screen.getByLabelText(t.secret)).toHaveValue("");
    expect(screen.getByLabelText(t.issuer)).toBeDisabled();
    expect(dialog).toHaveFocus();
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    act(() => { dialog.dispatchEvent(tab); });
    expect(tab.defaultPrevented).toBe(true);
    expect(dialog).toHaveFocus();

    await act(async () => pending.reject(new Error("do not show backend content")));
    expect(screen.getByRole("alert")).toHaveTextContent(t.error);
    expect(screen.queryByText("do not show backend content")).not.toBeInTheDocument();
    expect(screen.getByLabelText(t.secret)).toHaveValue("");
    expect(screen.getByLabelText(t.issuer)).toBeEnabled();
    fireEvent.submit(form);
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent(t.required);
    fireEvent.change(screen.getByLabelText(t.secret), { target: { value: "JBSWY3DPEHPK3PXP" } });
    await act(async () => fireEvent.submit(form));
    expect(handlers.onSave).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not reload details or clear a newly entered secret when the same account props refresh", async () => {
    const handlers = props();
    const { rerender } = render(<AccountDialog {...handlers} account={account} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    fireEvent.change(screen.getByLabelText(t.notes), { target: { value: "Edited notes" } });
    fireEvent.change(screen.getByLabelText(t.secret, { exact: false, selector: "input" }), { target: { value: "JBSWY3DPEHPK3PXP" } });
    rerender(<AccountDialog {...handlers} account={{ ...account }} t={{ ...t }} />);
    expect(accountCommands.details).toHaveBeenCalledTimes(1);
    const form = screen.getByRole("dialog").querySelector("form")!;
    await act(async () => fireEvent.submit(form));
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ notes: "Edited notes", secret: "JBSWY3DPEHPK3PXP" }));
  });

  it("accepts the backend-compatible maximum TOTP period", async () => {
    const handlers = props();
    render(<AccountDialog {...handlers} />);
    fillNewAccount();
    const period = screen.getByLabelText<HTMLInputElement>(t.period);
    expect(period).toHaveAttribute("max", "86400");
    fireEvent.change(period, { target: { value: "86400" } });
    await act(async () => fireEvent.submit(period.form!));
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ period_secs: 86400 }));
  });

  it.each(["0", "86401", "1.5"])("rejects invalid TOTP period %s even for a direct form submit", (value) => {
    const handlers = props();
    render(<AccountDialog {...handlers} />);
    fillNewAccount();
    const period = screen.getByLabelText<HTMLInputElement>(t.period);
    fireEvent.change(period, { target: { value } });
    fireEvent.submit(period.form!);
    expect(handlers.onSave).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(t.required);
  });
});
