import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getMessages } from "../../i18n";
import { backupMessages } from "../../i18n/backup";
import { workspaceMessages } from "../../i18n/workspace";
import type { AutoBackupStatus } from "../../lib/backup";
import { AutoBackupSettings } from "./AutoBackupSettings";

const api = vi.hoisted(() => ({ status: vi.fn(), configure: vi.fn() }));
vi.mock("../../lib/backup", () => ({ backupCommands: { status: api.status, configure: api.configure } }));

const t = getMessages("en"), b = backupMessages("en"), w = workspaceMessages("en");
const stored: AutoBackupStatus = { enabled: true, paused_locked: false, interval_hours: 6, retention: 3, last_run: null, last_attempt: null, last_error: null };

function mount(overrides: { onError?: (error: unknown) => void } = {}) {
  const props = { locale: "en" as const, onError: vi.fn(), notify: vi.fn(), ...overrides };
  render(<AutoBackupSettings {...props} />);
  return props;
}

beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.useRealTimers());

describe("automatic backup settings", () => {
  it("refreshes failure and recovery without overwriting drafts or passwords", async () => {
    api.status.mockResolvedValueOnce(stored)
      .mockResolvedValueOnce({ ...stored, last_run: 1_900_000_000, last_attempt: 2_000_000_000, last_error: "storage" })
      .mockResolvedValueOnce({ ...stored, last_run: 2_000_000_001, last_attempt: 2_000_000_001, last_error: null });
    mount();
    const hours = await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(hours, { target: { value: "12" } });
    fireEvent.change(screen.getByLabelText(t.fields.backupPassword), { target: { value: "unsaved-synthetic-password" } });
    fireEvent.click(screen.getByRole("button", { name: b.refresh }));
    expect(await screen.findByRole("alert")).toHaveTextContent(b.autoFailed);
    expect(screen.getByText(`${b.lastBackup}: ${new Date(1_900_000_000 * 1000).toLocaleString("en")}`)).toBeVisible();
    expect(hours).toHaveValue(12);
    expect(screen.getByLabelText(t.fields.backupPassword)).toHaveValue("unsaved-synthetic-password");
    fireEvent.click(screen.getByRole("button", { name: b.refresh }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(screen.getByText(`${b.lastBackup}: ${new Date(2_000_000_001 * 1000).toLocaleString("en")}`)).toBeVisible();
    expect(hours).toHaveValue(12);
    expect(api.configure).not.toHaveBeenCalled();
  });

  it("polls only status every 30 seconds and stops after unmount", async () => {
    vi.useFakeTimers();
    api.status.mockResolvedValue(stored);
    const view = render(<AutoBackupSettings locale="en" onError={vi.fn()} notify={vi.fn()} />);
    await act(async () => {});
    expect(api.status).toHaveBeenCalledTimes(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(30_000); });
    expect(api.status).toHaveBeenCalledTimes(2);
    expect(api.configure).not.toHaveBeenCalled();
    view.unmount();
    await act(async () => { await vi.advanceTimersByTimeAsync(90_000); });
    expect(api.status).toHaveBeenCalledTimes(2);
  });

  it("does not overlap slow polls or handle a late locked response after unmount", async () => {
    vi.useFakeTimers();
    let finish!: (value: AutoBackupStatus) => void;
    api.status.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const onError = vi.fn();
    const view = render(<AutoBackupSettings locale="en" onError={onError} notify={vi.fn()} />);
    await act(async () => { await vi.advanceTimersByTimeAsync(90_000); });
    expect(api.status).toHaveBeenCalledTimes(1);
    view.unmount();
    await act(async () => { finish({ ...stored, paused_locked: true }); });
    expect(onError).not.toHaveBeenCalled();
  });

  it("does not let a status response overwrite a newer configuration", async () => {
    let finish!: (value: AutoBackupStatus) => void;
    api.status.mockResolvedValueOnce(stored).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    api.configure.mockResolvedValue({ ...stored, enabled: false, last_run: 123 });
    mount();
    await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.click(screen.getByRole("button", { name: b.refresh }));
    fireEvent.click(screen.getByRole("button", { name: b.disable }));
    await screen.findByText(b.disabled);
    await act(async () => { finish({ ...stored, interval_hours: 999 }); });
    expect(screen.getByText(b.disabled)).toBeVisible();
    expect(screen.getByRole("spinbutton", { name: b.interval })).toHaveValue(6);
    expect(screen.getByRole("button", { name: b.disable })).toBeDisabled();
  });

  it("clears password inputs and propagates locked status", async () => {
    api.status.mockResolvedValueOnce(stored).mockResolvedValueOnce({ ...stored, paused_locked: true, enabled: false });
    const props = mount();
    await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(screen.getByLabelText(t.fields.backupPassword), { target: { value: "synthetic-password" } });
    fireEvent.click(screen.getByRole("button", { name: b.refresh }));
    await waitFor(() => expect(props.onError).toHaveBeenCalledWith({ code: "vault_locked" }));
    expect(screen.getByLabelText(t.fields.backupPassword)).toHaveValue("");
    expect(screen.getByRole("spinbutton", { name: b.interval })).toBeDisabled();
  });

  it("shows a load failure with a retry instead of a permanent loading tag", async () => {
    api.status.mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce({ ...stored, enabled: false });
    const props = mount();
    expect(screen.getByText(t.pages.loading)).toBeVisible();
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(w.autoLoadError);
    expect(props.onError).not.toHaveBeenCalled();
    expect(screen.queryByText(t.pages.loading)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: w.retry }));
    expect(await screen.findByRole("spinbutton", { name: b.interval })).toHaveValue(6);
    expect(api.status).toHaveBeenCalledTimes(2);
  });

  it("disables automatic backup with the saved values, not the unsaved draft", async () => {
    api.status.mockResolvedValue(stored);
    api.configure.mockResolvedValue({ ...stored, enabled: false });
    mount();
    const hours = await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(hours, { target: { value: "999" } });
    fireEvent.click(screen.getByRole("button", { name: b.disable }));
    expect(api.configure).toHaveBeenCalledExactlyOnceWith(false, "", 6, 3);
  });

  it("rejects invalid draft numbers locally without calling the backend", async () => {
    api.status.mockResolvedValue(stored);
    mount();
    const hours = await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(hours, { target: { value: "" } });
    fireEvent.submit(document.querySelector("form")!);
    expect(api.configure).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(t.status.invalid);
  });

  it("locks the fieldset so in-flight edits cannot overwrite a pending configuration", async () => {
    let finish!: (value: unknown) => void;
    api.status.mockResolvedValue(stored);
    api.configure.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const props = mount();
    await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(screen.getByLabelText(t.fields.backupPassword), { target: { value: "synthetic-password" } });
    fireEvent.change(screen.getByLabelText(b.passwordConfirm), { target: { value: "synthetic-password" } });
    const form = document.querySelector("form")!;
    fireEvent.submit(form);
    expect(api.configure).toHaveBeenCalledExactlyOnceWith(true, "synthetic-password", 6, 3);
    expect(screen.getByRole("spinbutton", { name: b.interval })).toBeDisabled();
    expect(screen.getByLabelText(t.fields.backupPassword)).toBeDisabled();
    expect(screen.getByRole("button", { name: b.disable })).toBeDisabled();
    fireEvent.submit(form);
    expect(api.configure).toHaveBeenCalledOnce();
    await act(async () => { finish({ ...stored }); });
    await waitFor(() => expect(screen.getByLabelText(t.fields.backupPassword)).toBeEnabled());
    expect(props.notify).toHaveBeenCalledWith(t.status.saved);
  });

  it("keeps the password mismatch guard before touching the backend", async () => {
    api.status.mockResolvedValue({ ...stored, enabled: false });
    mount();
    await screen.findByRole("spinbutton", { name: b.interval });
    fireEvent.change(screen.getByLabelText(t.fields.backupPassword), { target: { value: "one-password" } });
    fireEvent.change(screen.getByLabelText(b.passwordConfirm), { target: { value: "another-password" } });
    fireEvent.submit(document.querySelector("form")!);
    expect(api.configure).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(b.passwordMismatch);
  });
});