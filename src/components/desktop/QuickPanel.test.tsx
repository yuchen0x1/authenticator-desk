import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QuickPanel } from "./QuickPanel";

const state = vi.hoisted(() => ({
  locked: true,
  lockedHandler: undefined as (() => void) | undefined,
  status: vi.fn(),
  quickPanelAccounts: vi.fn(),
  codes: vi.fn(),
  copy: vi.fn(),
  showMain: vi.fn(),
  settings: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  accountCommands: { codes: state.codes, copy: state.copy },
  desktopCommands: {
    quickPanelAccounts: state.quickPanelAccounts,
    showMain: state.showMain,
  },
  vaultCommands: { status: state.status },
  settingsCommands: { get: state.settings },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_event: string, handler: () => void) => {
    state.lockedHandler = handler;
    return () => { state.lockedHandler = undefined; };
  }),
}));

const account = {
  id: "favorite-one",
  issuer: "Example",
  account_name: "alice",
  algorithm: "SHA1",
  digits: 6,
  kind: "totp",
  period_secs: 30,
  t0: "0",
  hotp_counter: "0",
  category_id: null,
  tags: [],
  notes_present: false,
  favorite: true,
  sort_order: 0,
  created_at: "2026-11-18T00:00:00Z",
  updated_at: "2026-11-18T00:00:00Z",
};

beforeEach(() => {
  state.locked = true;
  state.lockedHandler = undefined;
  state.status.mockReset().mockImplementation(async () => ({
    exists: true,
    locked: state.locked,
    generation: 1,
    revision: 1,
  }));
  state.quickPanelAccounts.mockReset().mockImplementation(async () => {
    if (state.locked) throw new Error("vault locked");
    return [account];
  });
  state.codes.mockReset().mockResolvedValue([{
    account_id: account.id,
    code: "654321",
    remaining_secs: 12,
    progress: 0.6,
    generated_at: 59,
    clock_offset_secs: 0,
    wall_clock_jump: false,
  }]);
  state.copy.mockReset().mockResolvedValue({ token: "opaque-token" });
  state.showMain.mockReset().mockResolvedValue(undefined);
  state.settings.mockReset().mockResolvedValue({ locale: "zh-CN" });
});

describe("favorite quick panel lock lifecycle", () => {
  it("reloads favorites on focus after unlock and clears them immediately on lock", async () => {
    render(<QuickPanel />);

    expect(await screen.findByText("已锁定")).toBeVisible();
    expect(state.quickPanelAccounts).not.toHaveBeenCalled();
    await waitFor(() => expect(state.lockedHandler).toBeTypeOf("function"));

    state.locked = false;
    act(() => window.dispatchEvent(new Event("focus")));
    expect(await screen.findByText("654321")).toBeVisible();

    state.locked = true;
    act(() => state.lockedHandler?.());
    expect(screen.getByText("已锁定")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();

    state.locked = false;
    act(() => window.dispatchEvent(new Event("focus")));
    expect(await screen.findByText("654321")).toBeVisible();
  });

  it("uses the saved vault language when showing favorites", async () => {
    state.locked = false;
    state.settings.mockResolvedValue({ locale: "en" });
    render(<QuickPanel />);

    expect(await screen.findByText("Favorites")).toBeVisible();
    expect(await screen.findByText("654321")).toBeVisible();
  });

  it("discards a late code response after the vault locks", async () => {
    let resolveCodes: ((value: unknown[]) => void) | undefined;
    state.locked = false;
    state.codes.mockImplementation(() => new Promise((resolve) => { resolveCodes = resolve; }));
    render(<QuickPanel />);

    await waitFor(() => expect(resolveCodes).toBeTypeOf("function"));
    act(() => state.lockedHandler?.());
    await act(async () => resolveCodes?.([{
      account_id: account.id,
      code: "654321",
      remaining_secs: 12,
      progress: 0.6,
      generated_at: 59,
      clock_offset_secs: 0,
      wall_clock_jump: false,
    }]));

    expect(screen.getByText("已锁定")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
  });
});
