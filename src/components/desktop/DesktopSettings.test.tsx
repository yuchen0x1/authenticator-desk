import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

  it("sends a test notification through the desktop command", async () => {
    const notify = vi.fn();
    render(<DesktopSettings locale="en" onError={vi.fn()} notify={notify} />);

    fireEvent.click(await screen.findByRole("button", { name: "Send test notification" }));

    await waitFor(() => expect(api.testNotification).toHaveBeenCalledOnce());
    expect(notify).toHaveBeenCalledWith("Test notification sent.");
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
});
