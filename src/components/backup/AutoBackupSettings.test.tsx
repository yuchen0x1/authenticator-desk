import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getMessages } from "../../i18n";
import { backupMessages } from "../../i18n/backup";
import { workspaceMessages } from "../../i18n/workspace";
import type { AutoBackupStatus } from "../../lib/backup";
import { AutoBackupSettings } from "./AutoBackupSettings";

const api = vi.hoisted(() => ({ status: vi.fn(), configure: vi.fn() }));
vi.mock("../../lib/backup", () => ({ backupCommands: { status: api.status, configure: api.configure } }));

const t = getMessages("en"), b = backupMessages("en"), w = workspaceMessages("en");
const stored: AutoBackupStatus = { enabled: true, paused_locked: false, interval_hours: 6, retention: 3, last_run: null };

function mount(overrides: { onError?: (error: unknown) => void } = {}) {
  const props = { locale: "en" as const, onError: vi.fn(), notify: vi.fn(), ...overrides };
  render(<AutoBackupSettings {...props} />);
  return props;
}

beforeEach(() => vi.resetAllMocks());

describe("automatic backup settings", () => {
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