import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DesktopCapabilities, DesktopConfig } from "../../lib/tauri";
import { DesktopSettings } from "./DesktopSettings";

const api = vi.hoisted(() => ({
  capabilities: vi.fn(),
  config: vi.fn(),
  saveConfig: vi.fn(),
  testNotification: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  desktopCommands: {
    capabilities: api.capabilities,
    config: api.config,
    saveConfig: api.saveConfig,
    testNotification: api.testNotification,
  },
}));

const config: DesktopConfig = {
  close_to_tray: false,
  minimize_to_tray: false,
  startup: false,
  notifications: true,
  show_shortcut: "Ctrl+Shift+A",
  quick_panel_shortcut: "Ctrl+Shift+Q",
  lock_shortcut: "Ctrl+Shift+L",
};

const capabilities: DesktopCapabilities = {
  tray: true,
  shortcuts: true,
  startup: false,
  notifications: true,
  clipboard: true,
  qr_image: true,
  camera: false,
  camera_reason: "Camera capture is not implemented.",
  warnings: [],
};

beforeEach(() => {
  api.capabilities.mockReset().mockResolvedValue(capabilities);
  api.config.mockReset().mockResolvedValue(config);
  api.saveConfig.mockReset().mockResolvedValue(undefined);
  api.testNotification.mockReset().mockResolvedValue(undefined);
});

describe("desktop integration settings", () => {
  it("loads desktop capabilities and saves the edited shortcut configuration", async () => {
    render(<DesktopSettings locale="en" onError={vi.fn()} notify={vi.fn()} />);

    expect(await screen.findByText("Desktop integration")).toBeVisible();
    expect(screen.getByLabelText("Quick panel shortcut")).toHaveValue("Ctrl+Shift+Q");
    expect(screen.getByText("Camera capture")).toBeVisible();
    expect(screen.getByText("Camera capture").parentElement).toHaveTextContent("Unavailable");
    expect(screen.getByText("Startup registration").parentElement).toHaveTextContent("Disabled");

    fireEvent.change(screen.getByLabelText("Quick panel shortcut"), {
      target: { value: "Ctrl+Alt+K" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save desktop settings" }));

    await waitFor(() => expect(api.saveConfig).toHaveBeenCalledWith({
      ...config,
      quick_panel_shortcut: "Ctrl+Alt+K",
    }));
  });

  it("does not start a second load while a retry request is still pending", async () => {
    let finish!: (value: DesktopConfig) => void;
    api.config.mockRejectedValueOnce(new Error("offline"));
    const onError = vi.fn();
    render(<DesktopSettings locale="en" onError={onError} notify={vi.fn()} />);
    const retry = await screen.findByRole("button", { name: "Retry" });
    api.config.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    act(() => {
      retry.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      retry.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(api.config).toHaveBeenCalledTimes(2);
    expect(api.capabilities).toHaveBeenCalledTimes(2);
    await act(async () => finish(config));
    expect(await screen.findByText("Desktop integration")).toBeVisible();
  });

  it("blocks reentrant saves and disables every draft control until the save returns", async () => {
    let finish!: () => void;
    api.saveConfig.mockReturnValue(new Promise<void>((resolve) => { finish = resolve; }));
    render(<DesktopSettings locale="en" onError={vi.fn()} notify={vi.fn()} />);
    await screen.findByText("Desktop integration");
    const shortcut = screen.getByLabelText("Quick panel shortcut");
    fireEvent.change(shortcut, { target: { value: "Ctrl+Alt+K" } });
    const save = screen.getByRole("button", { name: "Save desktop settings" });
    act(() => {
      save.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      save.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(api.saveConfig).toHaveBeenCalledExactlyOnceWith({ ...config, quick_panel_shortcut: "Ctrl+Alt+K" });
    expect(save).toBeDisabled();
    expect(shortcut).toBeDisabled();
    await act(async () => finish());
    await waitFor(() => expect(save).toBeEnabled());
    expect(shortcut).toHaveValue("Ctrl+Alt+K");
  });

  it("blocks reentrant notification tests and reports non-vault failures locally", async () => {
    let finish!: (value?: unknown) => void;
    api.testNotification.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const notify = vi.fn(), onError = vi.fn();
    render(<DesktopSettings locale="en" onError={onError} notify={notify} />);
    const testButton = await screen.findByRole("button", { name: "Send test notification" });
    act(() => {
      testButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      testButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(api.testNotification).toHaveBeenCalledOnce();
    expect(testButton).toBeDisabled();
    await act(async () => finish());
    await waitFor(() => expect(testButton).toBeEnabled());
    expect(notify).toHaveBeenCalledWith("Test notification sent.");

    api.testNotification.mockRejectedValueOnce(new Error("offline"));
    fireEvent.click(testButton);
    expect(await screen.findByRole("alert")).toHaveTextContent("Action failed");
    expect(onError).not.toHaveBeenCalled();
  });

  it("requires saving an enabled notification setting before testing it", async () => {
    api.config.mockResolvedValueOnce({ ...config, notifications: false });
    render(<DesktopSettings locale="en" onError={vi.fn()} notify={vi.fn()} />);

    const notificationToggle = await screen.findByLabelText("Enable desktop notifications");
    const testButton = screen.getByRole("button", { name: "Send test notification" });
    expect(testButton).toBeDisabled();

    fireEvent.click(notificationToggle);
    expect(testButton).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Save desktop settings" }));

    await waitFor(() => expect(api.saveConfig).toHaveBeenCalledWith({ ...config, notifications: true }));
    expect(testButton).toBeEnabled();
    fireEvent.click(testButton);
    await waitFor(() => expect(api.testNotification).toHaveBeenCalledOnce());
  });

  it("sends vault-locked failures to the global boundary without duplicating local feedback", async () => {
    const onError = vi.fn();
    api.saveConfig.mockRejectedValueOnce({ code: "vault_locked" });
    render(<DesktopSettings locale="en" onError={onError} notify={vi.fn()} />);
    await screen.findByText("Desktop integration");
    fireEvent.click(screen.getByRole("button", { name: "Save desktop settings" }));
    await waitFor(() => expect(onError).toHaveBeenCalledWith({ code: "vault_locked" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
