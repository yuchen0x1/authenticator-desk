import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UpdateSettings } from "./UpdateSettings";

const state = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as Array<{ onmessage?: (value: { received: number; total: number }) => void }>,
}));

vi.mock("../../lib/tauri", () => ({ tauriInvoke: state.invoke }));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class MockChannel {
    onmessage?: (value: { received: number; total: number }) => void;
    constructor() { state.channels.push(this); }
  },
}));

const version = { version: "1.0.0", available: true };
const release = { version: "1.1.0", date: "2026-11-20", notes: "Security fixes", bytes: 2 * 1024 * 1024 };
const dialogPrototype = HTMLDialogElement.prototype as HTMLDialogElement & { showModal?: () => void; close?: () => void };
const originalShowModal = dialogPrototype.showModal;
const originalClose = dialogPrototype.close;

beforeEach(() => {
  state.invoke.mockReset();
  state.channels.length = 0;
  state.invoke.mockImplementation((command: string) => command === "update_version" ? Promise.resolve(version) : undefined);
  Object.defineProperty(dialogPrototype, "showModal", { configurable: true, writable: true, value: function (this: HTMLDialogElement) { this.setAttribute("open", ""); } });
  Object.defineProperty(dialogPrototype, "close", { configurable: true, writable: true, value: function (this: HTMLDialogElement) {
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  } });
});

afterEach(() => {
  if (originalShowModal) Object.defineProperty(dialogPrototype, "showModal", { configurable: true, writable: true, value: originalShowModal });
  else Reflect.deleteProperty(dialogPrototype, "showModal");
  if (originalClose) Object.defineProperty(dialogPrototype, "close", { configurable: true, writable: true, value: originalClose });
  else Reflect.deleteProperty(dialogPrototype, "close");
});

describe("update settings", () => {
  it("prevents reentrant update checks and exposes an accessible current status", async () => {
    let finish!: (value: null) => void;
    state.invoke.mockImplementation((command: string) => {
      if (command === "update_version") return Promise.resolve(version);
      if (command === "check_update") return new Promise((resolve) => { finish = resolve; });
      return Promise.resolve(undefined);
    });
    render(<UpdateSettings locale="en" />);
    const check = await screen.findByRole("button", { name: "Check for updates" });
    act(() => {
      check.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      check.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    expect(state.invoke.mock.calls.filter(([command]) => command === "check_update")).toHaveLength(1);
    expect(check).toBeDisabled();
    expect(screen.getByText("Checking…")).toBeVisible();
    await act(async () => finish(null));
    expect(await screen.findByRole("status")).toHaveTextContent("You have the latest version.");
    expect(check).toBeEnabled();
  });

  it("keeps the native dialog flow and restores focus after dismissing an offer", async () => {
    state.invoke.mockImplementation((command: string) => {
      if (command === "update_version") return Promise.resolve(version);
      if (command === "check_update") return Promise.resolve(release);
      return Promise.resolve(undefined);
    });
    render(<UpdateSettings locale="en" />);
    const check = await screen.findByRole("button", { name: "Check for updates" });
    fireEvent.click(check);
    const dialog = await screen.findByRole("dialog", { name: /Update available/ });
    expect(dialog).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Later" }));
    await waitFor(() => expect(check).toHaveFocus());
    expect(screen.queryByText("Security fixes")).not.toBeInTheDocument();
  });

  it("prevents reentrant installs, reports progress, and keeps install failures accessible", async () => {
    let rejectInstall!: (reason: unknown) => void;
    state.invoke.mockImplementation((command: string) => {
      if (command === "update_version") return Promise.resolve(version);
      if (command === "check_update") return Promise.resolve(release);
      if (command === "install_update") return new Promise((_resolve, reject) => { rejectInstall = reject; });
      return Promise.resolve(undefined);
    });
    render(<UpdateSettings locale="en" />);
    fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    await screen.findByRole("dialog", { name: /Update available/ });
    const install = screen.getByRole("button", { name: "Install and restart" });
    fireEvent.click(install);
    fireEvent.click(install);
    expect(state.invoke).toHaveBeenCalledTimes(3);
    expect(install).toBeDisabled();
    expect(state.channels).toHaveLength(1);
    act(() => state.channels[0].onmessage?.({ received: 1024 * 1024, total: release.bytes }));
    expect(screen.getByRole("progressbar")).toHaveValue(1024 * 1024);
    await act(async () => rejectInstall("integrity"));
    expect(await screen.findByRole("alert")).toHaveTextContent("The release metadata or package could not be verified.");
    expect(install).toBeEnabled();
  });
});
