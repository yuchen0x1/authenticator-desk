import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceProps } from "../App";
import { getMessages } from "../i18n";
import { changeMasterPassword } from "../lib/tauri";
import { SettingsPage } from "./SettingsPage";

vi.mock("../lib/tauri", () => ({ changeMasterPassword: vi.fn() }));
vi.mock("../components/backup/AutoBackupSettings", () => ({ AutoBackupSettings: () => null }));
vi.mock("../components/desktop/DesktopSettings", () => ({ DesktopSettings: () => null }));
vi.mock("../components/desktop/UpdateSettings", () => ({ UpdateSettings: () => null }));

const t = getMessages("en");
const settings: WorkspaceProps["settings"] = {
  locale: "en", theme: "dark", idle_timeout_secs: 900, clipboard_ttl_secs: 30, hide_codes: false,
  view_mode: "cards", animations: false, compact: false, font_size: 14, default_algorithm: "SHA1",
  default_digits: 6, default_period_secs: 30, close_to_tray: false, minimize_to_tray: false,
  startup: false, backup_enabled: false, backup_interval_hours: 24, backup_retention: 7, show_next_code: false,
};

function workspace(): WorkspaceProps {
  return {
    locale: "en", accounts: [], categories: [], codes: [], settings,
    refresh: vi.fn(async () => {}), notify: vi.fn(), onError: vi.fn(), onAdd: vi.fn(), onEdit: vi.fn(),
    navigate: vi.fn(), saveSettings: vi.fn(async () => {}),
  };
}

const extraNodes = new Set<HTMLElement>();
function bodySibling(inert?: string) {
  const element = document.createElement("aside");
  if (inert !== undefined) element.setAttribute("inert", inert);
  document.body.append(element);
  extraNodes.add(element);
  return element;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function settingsForm() {
  return screen.getByLabelText<HTMLInputElement>(t.fields.idleTimeout).form!;
}
function fillPasswords(dialog: HTMLElement) {
  fireEvent.change(within(dialog).getByLabelText(t.fields.currentPassword), { target: { value: "old-test-password" } });
  fireEvent.change(within(dialog).getByLabelText(t.fields.newPassword), { target: { value: "new-test-password" } });
  fireEvent.change(within(dialog).getByLabelText(t.shell.confirmPassword), { target: { value: "new-test-password" } });
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(changeMasterPassword).mockReset().mockResolvedValue(undefined);
});
afterEach(() => {
  for (const element of extraNodes) element.remove();
  extraNodes.clear();
  vi.restoreAllMocks();
});

describe("security timeout settings", () => {
  it("does not save a blank idle timeout", () => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    const input = screen.getByRole<HTMLInputElement>("spinbutton", { name: t.fields.idleTimeout });
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.submit(input.form!);

    expect(props.saveSettings).not.toHaveBeenCalled();
    expect(props.onError).toHaveBeenCalledWith({ code: "invalid_input" });
  });

  it("accepts explicit idle-timeout disable and the clipboard maximum of 300 seconds", () => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    const idleInput = screen.getByRole<HTMLInputElement>("spinbutton", { name: t.fields.idleTimeout });
    const clipboardInput = screen.getByRole<HTMLInputElement>("spinbutton", { name: t.fields.clipboardTtl });
    expect(clipboardInput).toHaveAttribute("max", "300");
    fireEvent.click(screen.getByRole("checkbox", { name: "Disable automatic locking" }));
    expect(idleInput).toBeDisabled();
    fireEvent.change(clipboardInput, { target: { value: "300" } });
    fireEvent.submit(idleInput.form!);

    expect(props.saveSettings).toHaveBeenCalledExactlyOnceWith({
      ...settings, idle_timeout_secs: 0, clipboard_ttl_secs: 300,
    });
    expect(props.onError).not.toHaveBeenCalled();
  });

  it("rejects a clipboard timeout above the 300-second cap", () => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    const input = screen.getByRole<HTMLInputElement>("spinbutton", { name: t.fields.clipboardTtl });
    fireEvent.change(input, { target: { value: "301" } });
    fireEvent.submit(input.form!);

    expect(props.saveSettings).not.toHaveBeenCalled();
    expect(props.onError).toHaveBeenCalledWith({ code: "invalid_input" });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("accepts the maximum default period and explicit clipboard cleanup disable", async () => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    fireEvent.click(screen.getByRole("checkbox", { name: "Disable clipboard cleanup" }));
    expect(screen.getByLabelText(t.fields.clipboardTtl)).toBeDisabled();
    const period = screen.getByLabelText(t.settings.defaultPeriod);
    expect(period).toHaveAttribute("max", "86400");
    fireEvent.change(period, { target: { value: "86400" } });
    await act(async () => fireEvent.submit(settingsForm()));
    expect(props.saveSettings).toHaveBeenCalledExactlyOnceWith({ ...settings, clipboard_ttl_secs: 0, default_period_secs: 86400 });
  });
});

describe("settings save lifecycle", () => {
  it("merges dirty fields over the latest props without replacing unedited timers or other settings", async () => {
    const props = workspace();
    const { rerender } = render(<SettingsPage {...props} />);
    fireEvent.change(screen.getByLabelText(t.fields.theme), { target: { value: "light" } });
    fireEvent.change(screen.getByLabelText(t.fields.idleTimeout), { target: { value: "120" } });
    const latest = { ...settings, idle_timeout_secs: 600, clipboard_ttl_secs: 90, default_period_secs: 45, startup: true, backup_enabled: true };
    rerender(<SettingsPage {...props} settings={latest} />);
    expect(screen.getByLabelText(t.fields.idleTimeout)).toHaveValue(120);
    expect(screen.getByLabelText(t.fields.clipboardTtl)).toHaveValue(90);
    expect(screen.getByLabelText(t.settings.defaultPeriod)).toHaveValue(45);
    await act(async () => fireEvent.submit(settingsForm()));
    expect(props.saveSettings).toHaveBeenCalledExactlyOnceWith({ ...latest, theme: "light", idle_timeout_secs: 120 });
  });

  it("guards same-tick duplicate saves, disables editing, and retains failed dirty changes for a latest-props retry", async () => {
    const pending = deferred<void>();
    const props = workspace();
    vi.mocked(props.saveSettings).mockReturnValueOnce(pending.promise);
    const { rerender } = render(<SettingsPage {...props} />);
    fireEvent.change(screen.getByLabelText(t.fields.theme), { target: { value: "light" } });
    const form = settingsForm();
    act(() => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });
    expect(props.saveSettings).toHaveBeenCalledTimes(1);
    expect(form).toHaveAttribute("aria-busy", "true");
    expect(screen.getByLabelText(t.fields.theme)).toBeDisabled();
    expect(screen.getByRole("button", { name: t.actions.changePassword })).toBeDisabled();
    fireEvent.change(screen.getByLabelText(t.fields.theme), { target: { value: "dark" } });
    const latest = { ...settings, idle_timeout_secs: 0, clipboard_ttl_secs: 60, default_period_secs: 90, startup: true };
    rerender(<SettingsPage {...props} settings={latest} />);
    const reason = { code: "storage", message: "private backend content" };
    await act(async () => pending.reject(reason));
    expect(screen.getByRole("alert")).toHaveTextContent(t.status.error);
    expect(screen.queryByText(reason.message)).not.toBeInTheDocument();
    expect(props.onError).not.toHaveBeenCalled();
    expect(props.notify).not.toHaveBeenCalled();
    expect(screen.getByLabelText(t.fields.theme)).toHaveValue("light");
    expect(screen.getByLabelText(t.fields.theme)).toBeEnabled();
    await act(async () => fireEvent.submit(form));
    expect(props.saveSettings).toHaveBeenLastCalledWith({ ...latest, theme: "light" });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    const lock = { code: "vault_locked" };
    vi.mocked(props.saveSettings).mockRejectedValueOnce(lock);
    await act(async () => fireEvent.submit(form));
    expect(props.saveSettings).toHaveBeenCalledTimes(3);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(lock);
  });

  it("releases saved dirty fields to newer props rather than replaying a completed stale snapshot", async () => {
    const pending = deferred<void>();
    const props = workspace();
    vi.mocked(props.saveSettings).mockReturnValueOnce(pending.promise);
    const { rerender } = render(<SettingsPage {...props} />);
    fireEvent.change(screen.getByLabelText(t.fields.theme), { target: { value: "light" } });
    fireEvent.change(screen.getByLabelText(t.settings.defaultPeriod), { target: { value: "60" } });
    fireEvent.submit(settingsForm());
    const latest = { ...settings, theme: "dark", compact: true, idle_timeout_secs: 600, default_period_secs: 120, startup: true };
    rerender(<SettingsPage {...props} settings={latest} />);
    expect(screen.getByLabelText(t.fields.theme)).toHaveValue("light");
    expect(screen.getByLabelText(t.settings.defaultPeriod)).toHaveValue(60);
    await act(async () => pending.resolve());
    expect(screen.getByLabelText(t.fields.theme)).toHaveValue("dark");
    expect(screen.getByLabelText(t.settings.defaultPeriod)).toHaveValue(120);
    await act(async () => fireEvent.submit(settingsForm()));
    expect(props.saveSettings).toHaveBeenLastCalledWith(latest);
  });

  it("retains an acknowledged save until new props arrive, without requiring an API return value", async () => {
    const props = workspace();
    render(<SettingsPage {...props} />);
    fireEvent.change(screen.getByLabelText(t.fields.theme), { target: { value: "light" } });
    await act(async () => fireEvent.submit(settingsForm()));
    expect(screen.getByLabelText(t.fields.theme)).toHaveValue("light");
    fireEvent.change(screen.getByLabelText(t.settings.defaultDigits), { target: { value: "8" } });
    await act(async () => fireEvent.submit(settingsForm()));
    expect(props.saveSettings).toHaveBeenLastCalledWith({ ...settings, theme: "light", default_digits: 8 });
  });
});

describe("password modal", () => {
  it("isolates the whole background including new body portals and restores previous inert attributes before returning focus", async () => {
    const preexisting = bodySibling("retained"), background = bodySibling();
    const { container } = render(<SettingsPage {...workspace()} />);
    const trigger = screen.getByRole("button", { name: t.actions.changePassword });
    fireEvent.click(trigger);
    const dialog = screen.getByRole("dialog");
    expect(container).not.toContainElement(dialog);
    expect(container).toHaveAttribute("inert");
    expect(background).toHaveAttribute("inert");
    expect(dialog.closest("[inert]")).toBeNull();
    expect(within(dialog).getByLabelText(t.fields.currentPassword)).toHaveFocus();
    const newPortal = bodySibling();
    await waitFor(() => expect(newPortal).toHaveAttribute("inert"));
    let inertOnFocus: Element | null | undefined;
    trigger.addEventListener("focus", () => { inertOnFocus = trigger.closest("[inert]"); }, { once: true });
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(container).not.toHaveAttribute("inert");
    expect(background).not.toHaveAttribute("inert");
    expect(newPortal).not.toHaveAttribute("inert");
    expect(preexisting).toHaveAttribute("inert", "retained");
    expect(trigger).toHaveFocus();
    expect(inertOnFocus).toBeNull();
    const afterClose = bodySibling();
    await act(async () => {});
    expect(afterClose).not.toHaveAttribute("inert");
  });

  it("blocks reentrant password updates and dismissal, clears credentials, and keeps failures local and retryable", async () => {
    const pending = deferred<void>();
    vi.mocked(changeMasterPassword).mockReturnValueOnce(pending.promise);
    const localWrite = vi.spyOn(localStorage, "setItem"), sessionWrite = vi.spyOn(sessionStorage, "setItem");
    const props = workspace();
    const { container } = render(<SettingsPage {...props} />);
    const trigger = screen.getByRole("button", { name: t.actions.changePassword });
    fireEvent.click(trigger);
    const dialog = screen.getByRole("dialog"), form = dialog.querySelector("form")!;
    fillPasswords(dialog);
    act(() => {
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
      fireEvent.keyDown(dialog, { key: "Escape" });
    });
    expect(changeMasterPassword).toHaveBeenCalledExactlyOnceWith("old-test-password", "new-test-password");
    for (const input of dialog.querySelectorAll("input")) {
      expect(input).toHaveValue("");
      expect(input).toBeDisabled();
    }
    expect(within(dialog).getByRole("button", { name: t.actions.cancel })).toBeDisabled();
    expect(dialog).toHaveFocus();
    expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
    expect(dialog).toHaveFocus();
    expect(container).toHaveAttribute("inert");

    const reason = { code: "authentication_failed", message: "private backend content" };
    await act(async () => pending.reject(reason));
    expect(within(dialog).getByRole("alert")).toHaveTextContent(t.status.error);
    expect(screen.queryByText(reason.message)).not.toBeInTheDocument();
    expect(props.onError).not.toHaveBeenCalled();
    expect(props.notify).not.toHaveBeenCalled();
    expect(within(dialog).getByLabelText(t.fields.currentPassword)).toHaveFocus();
    for (const input of dialog.querySelectorAll("input")) expect(input).toHaveValue("");
    const lock = { code: "vault_locked" };
    vi.mocked(changeMasterPassword).mockRejectedValueOnce(lock);
    fillPasswords(dialog);
    await act(async () => fireEvent.submit(form));
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(lock);
    expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
    fillPasswords(dialog);
    await act(async () => fireEvent.submit(form));
    expect(changeMasterPassword).toHaveBeenCalledTimes(3);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(props.notify).toHaveBeenCalledExactlyOnceWith(t.status.saved);
    expect(container).not.toHaveAttribute("inert");
    expect(trigger).toHaveFocus();
    expect(localWrite).not.toHaveBeenCalled();
    expect(sessionWrite).not.toHaveBeenCalled();
    fireEvent.click(trigger);
    for (const input of screen.getByRole("dialog").querySelectorAll("input")) expect(input).toHaveValue("");
  });

  it("restores isolation on unmount and ignores a late password completion", async () => {
    const pending = deferred<void>();
    vi.mocked(changeMasterPassword).mockReturnValueOnce(pending.promise);
    const background = bodySibling();
    const props = workspace();
    const { unmount } = render(<SettingsPage {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t.actions.changePassword }));
    const dialog = screen.getByRole("dialog");
    fillPasswords(dialog);
    fireEvent.submit(dialog.querySelector("form")!);
    expect(background).toHaveAttribute("inert");
    unmount();
    expect(background).not.toHaveAttribute("inert");
    await act(async () => pending.resolve());
    expect(props.notify).not.toHaveBeenCalled();
    expect(props.onError).not.toHaveBeenCalled();
  });
});
