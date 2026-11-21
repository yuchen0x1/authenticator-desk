import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getMessages, type Locale } from "../../i18n";
import { accountUiMessages } from "../../i18n/accountUi";
import { accountCommands } from "../../lib/tauri";
import type { AccountDetails, AccountView } from "../../lib/types";
import { AccountDialog } from "./AccountDialog";

vi.mock("../../lib/tauri", () => ({ accountCommands: { details: vi.fn() } }));

const messages = getMessages("en");
// Public RFC 4648 Base32 test vector for "foobar", not an account credential.
const replacementSecret = "MZXW6YTBOI";
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
  it("edits the authenticated details snapshot instead of stale list metadata", async () => {
    const latest: AccountView = { ...account, issuer:"Current service", account_name:"current-user", kind:"hotp", hotp_counter:"9007199254740993", algorithm:"SHA256", digits:8, period_secs:60, t0:"123", tags:["current"], favorite:true };
    vi.mocked(accountCommands.details).mockResolvedValueOnce({account:latest, notes:"Current notes"});
    const handlers = props();
    render(<AccountDialog {...handlers} account={{...account, kind:"hotp", hotp_counter:"0"}} />);
    await waitFor(() => expect(screen.getByRole("button", {name:t.save})).toBeEnabled());
    expect(screen.getByLabelText(t.counter)).toHaveValue(latest.hotp_counter);
    expect(screen.getByLabelText(t.issuer)).toHaveValue(latest.issuer);
    fireEvent.change(screen.getByLabelText(t.notes), {target:{value:"Updated notes only"}});
    await act(async () => fireEvent.submit(screen.getByRole("dialog").querySelector("form")!));
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({issuer:latest.issuer, account_name:latest.account_name, kind:"hotp", hotp_counter:latest.hotp_counter, algorithm:"SHA256", digits:8, period_secs:60, t0:"123", tags:["current"], favorite:true, notes:"Updated notes only", secret:null}));
  });

  it("rejects details for a different account and allows retry without enabling a stale edit", async () => {
    vi.mocked(accountCommands.details).mockResolvedValueOnce({account:{...account,id:"another-account"},notes:"Unrelated notes"});
    const handlers = props();
    render(<AccountDialog {...handlers} account={account} />);
    expect(await screen.findByRole("alert")).toHaveTextContent(t.detailsLoadError);
    expect(screen.getByRole("button", {name:t.save})).toBeDisabled();
    expect(screen.getByLabelText(t.notes)).not.toHaveValue("Unrelated notes");
    fireEvent.click(screen.getByRole("button", {name:t.retry}));
    await waitFor(() => expect(screen.getByRole("button", {name:t.save})).toBeEnabled());
    expect(screen.getByLabelText(t.notes)).toHaveValue("Existing private notes");
  });

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
    expect(screen.queryByRole("button", { name: accountUiMessages("en").cancelSecretReplacement })).not.toBeInTheDocument();
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

  it("blocks an empty replacement retry after a failed edit, including a same-id refresh, until the secret is re-entered", async () => {
    const pending = deferred<void>();
    const handlers = props();
    const ui = accountUiMessages("en");
    vi.mocked(handlers.onSave).mockReturnValueOnce(pending.promise);
    const { rerender } = render(<AccountDialog {...handlers} account={account} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    const secret = screen.getByLabelText<HTMLInputElement>(t.secret, { exact: false, selector: "input" });
    const form = secret.form!;
    fireEvent.change(secret, { target: { value: "JBSWY3DPEHPK3PXP" } });
    fireEvent.change(screen.getByLabelText(t.tags), { target: { value: "x".repeat(129) } });
    fireEvent.submit(form);
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ secret: "JBSWY3DPEHPK3PXP" }));
    expect(secret).toHaveValue("");
    const cancelReplacement = screen.getByRole("button", { name: ui.cancelSecretReplacement });
    expect(cancelReplacement).toBeDisabled();
    fireEvent.click(cancelReplacement);
    expect(handlers.onClose).not.toHaveBeenCalled();

    await act(async () => pending.reject(new Error("tag too long: private backend detail")));
    expect(screen.getByRole("alert")).toHaveTextContent(t.error);
    expect(screen.queryByText("tag too long: private backend detail")).not.toBeInTheDocument();
    expect(secret).toHaveValue("");
    expect(secret).toHaveAccessibleDescription(ui.secretReplacementRequired);
    expect(secret).not.toHaveAttribute("placeholder", t.secretKeep);
    expect(screen.getByLabelText(t.issuer)).toHaveFocus();
    fireEvent.change(screen.getByLabelText(t.tags), { target: { value: "fixed" } });
    rerender(<AccountDialog {...handlers} account={{ ...account, updated_at: "2026-11-21T00:00:00Z" }} t={{ ...t }} />);
    expect(accountCommands.details).toHaveBeenCalledTimes(1);
    fireEvent.submit(form);
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent(ui.secretReplacementRequired);
    fireEvent.change(secret, { target: { value: "   " } });
    fireEvent.submit(form);
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent(ui.secretReplacementRequired);

    fireEvent.change(secret, { target: { value: ` ${replacementSecret} ` } });
    await act(async () => fireEvent.submit(form));
    expect(handlers.onSave).toHaveBeenCalledTimes(2);
    expect(handlers.onSave).toHaveBeenLastCalledWith(expect.objectContaining({ secret: replacementSecret, tags: ["fixed"] }));
    expect(secret).toHaveValue("");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps replacement intent across a synchronous save failure and immediate retry before rerender", async () => {
    const handlers = props();
    vi.mocked(handlers.onSave).mockImplementationOnce(() => { throw new Error("save failed synchronously"); });
    render(<AccountDialog {...handlers} account={account} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    const secret = screen.getByLabelText<HTMLInputElement>(t.secret, { exact: false, selector: "input" });
    fireEvent.change(secret, { target: { value: "JBSWY3DPEHPK3PXP" } });
    act(() => {
      secret.form!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      secret.form!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(secret).toHaveValue("");
    expect(screen.getByRole("alert")).toHaveTextContent(accountUiMessages("en").secretReplacementRequired);
  });

  it.each<Locale>(["zh-CN", "zh-TW", "en", "ja"])("allows explicit cancellation of replacement to preserve the current secret in %s", async (locale) => {
    const messages = getMessages(locale);
    const localized = { ...t, ...messages.fields, ...messages.actions, ...messages.status };
    const ui = accountUiMessages(locale);
    const handlers = props();
    vi.mocked(handlers.onSave).mockRejectedValueOnce(new Error("save failed"));
    render(<AccountDialog {...handlers} account={account} t={localized} />);
    await waitFor(() => expect(screen.getByLabelText(localized.notes)).toBeEnabled());
    const secret = screen.getByLabelText<HTMLInputElement>(localized.secret, { exact: false, selector: "input" });
    fireEvent.change(secret, { target: { value: "JBSWY3DPEHPK3PXP" } });
    await act(async () => fireEvent.submit(secret.form!));
    expect(secret).toHaveValue("");
    expect(secret).toHaveAccessibleDescription(ui.secretReplacementRequired);
    fireEvent.submit(secret.form!);
    expect(handlers.onSave).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent(ui.secretReplacementRequired);

    fireEvent.change(secret, { target: { value: replacementSecret } });
    const cancelReplacement = screen.getByRole("button", { name: ui.cancelSecretReplacement });
    cancelReplacement.focus();
    fireEvent.click(cancelReplacement);
    expect(handlers.onClose).not.toHaveBeenCalled();
    expect(secret).toHaveValue("");
    expect(secret).toHaveFocus();
    expect(secret).toHaveAccessibleDescription(localized.secretKeep);
    expect(secret).toHaveAttribute("placeholder", localized.secretKeep);
    expect(screen.queryByRole("button", { name: ui.cancelSecretReplacement })).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await act(async () => fireEvent.submit(secret.form!));
    expect(handlers.onSave).toHaveBeenCalledTimes(2);
    expect(handlers.onSave).toHaveBeenLastCalledWith(expect.objectContaining({ secret: null, notes: "Existing private notes" }));
  });

  it.each(["button", "escape", "backdrop"])("clears replacement intent and secrets when closed via %s and reopened", async (method) => {
    const handlers = props();
    const ui = accountUiMessages("en");
    vi.mocked(handlers.onSave).mockRejectedValueOnce(new Error("save failed"));
    const { unmount } = render(<AccountDialog {...handlers} account={account} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    const secret = screen.getByLabelText<HTMLInputElement>(t.secret, { exact: false, selector: "input" });
    fireEvent.change(secret, { target: { value: "JBSWY3DPEHPK3PXP" } });
    await act(async () => fireEvent.submit(secret.form!));
    fireEvent.change(secret, { target: { value: replacementSecret } });
    const dialog = screen.getByRole("dialog");
    if (method === "button") fireEvent.click(screen.getAllByRole("button", { name: t.cancel })[0]);
    else if (method === "escape") fireEvent.keyDown(dialog, { key: "Escape" });
    else fireEvent.mouseDown(dialog.parentElement!);
    expect(handlers.onClose).toHaveBeenCalledTimes(1);
    expect(secret).toHaveValue("");
    expect(screen.queryByRole("button", { name: ui.cancelSecretReplacement })).not.toBeInTheDocument();
    unmount();

    render(<AccountDialog {...handlers} account={{ ...account }} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    const reopenedSecret = screen.getByLabelText<HTMLInputElement>(t.secret, { exact: false, selector: "input" });
    expect(reopenedSecret).toHaveValue("");
    expect(reopenedSecret).toHaveAccessibleDescription(t.secretKeep);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: ui.cancelSecretReplacement })).not.toBeInTheDocument();
    await act(async () => fireEvent.submit(reopenedSecret.form!));
    expect(handlers.onSave).toHaveBeenCalledTimes(2);
    expect(handlers.onSave).toHaveBeenLastCalledWith(expect.objectContaining({ secret: null }));
  });

  it("does not require a secret when retrying a failed metadata-only edit", async () => {
    const handlers = props();
    vi.mocked(handlers.onSave).mockRejectedValueOnce(new Error("save failed"));
    render(<AccountDialog {...handlers} account={account} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    const form = screen.getByRole("dialog").querySelector("form")!;
    await act(async () => fireEvent.submit(form));
    expect(screen.queryByRole("button", { name: accountUiMessages("en").cancelSecretReplacement })).not.toBeInTheDocument();
    await act(async () => fireEvent.submit(form));
    expect(handlers.onSave).toHaveBeenCalledTimes(2);
    expect(handlers.onSave).toHaveBeenLastCalledWith(expect.objectContaining({ secret: null }));
  });

  it("allows editing an imported account with an empty optional issuer", async () => {
    const imported = {...account, issuer:""};
    vi.mocked(accountCommands.details).mockResolvedValueOnce({account:imported, notes:"Existing notes"});
    const handlers = props();
    render(<AccountDialog {...handlers} account={imported} />);
    await waitFor(() => expect(screen.getByLabelText(t.notes)).toBeEnabled());
    expect(screen.getByLabelText(t.issuer)).not.toBeRequired();
    fireEvent.change(screen.getByLabelText(t.notes), {target:{value:"Edited without inventing a service"}});
    await act(async () => fireEvent.submit(screen.getByRole("dialog").querySelector("form")!));
    expect(handlers.onSave).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({issuer:"", account_name:account.account_name, notes:"Edited without inventing a service", secret:null}));
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
