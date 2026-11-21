import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QuickPanel } from "./QuickPanel";

const state = vi.hoisted(() => ({
  locked: true,
  hideCodes: false,
  generation: 1,
  revision: 1,
  handlers: new Map<string, () => void>(),
  status: vi.fn(),
  quickPanelAccounts: vi.fn(),
  codes: vi.fn(),
  copy: vi.fn(),
  showMain: vi.fn(),
  settings: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  accountCommands: { codes: state.codes, copy: state.copy },
  desktopCommands: { quickPanelAccounts: state.quickPanelAccounts, showMain: state.showMain },
  vaultCommands: { status: state.status },
  settingsCommands: { get: state.settings },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: () => void) => {
    state.handlers.set(event, handler);
    return () => { if(state.handlers.get(event) === handler) state.handlers.delete(event); };
  }),
}));

const account = {
  id: "favorite-one", issuer: "Example", account_name: "alice", algorithm: "SHA1", digits: 6,
  kind: "totp", period_secs: 30, t0: "0", hotp_counter: "0", category_id: null, tags: [],
  notes_present: false, favorite: true, sort_order: 0, created_at: "2026-11-18T00:00:00Z", updated_at: "2026-11-18T00:00:00Z",
};
const code = (value="654321",generatedAt=Math.floor(Date.now()/1000)) => ({
  account_id: account.id, code: value, remaining_secs: 12, progress: 0.6, generated_at: generatedAt,
  clock_offset_secs: 0, wall_clock_jump: false,
});
function deferred<T>() { let resolve!: (value:T)=>void; const promise=new Promise<T>((done)=>{resolve=done;});return {promise,resolve}; }
beforeEach(() => {
  state.locked = true; state.hideCodes = false; state.generation = 1; state.revision = 1; state.handlers.clear();
  state.status.mockReset().mockImplementation(async () => ({exists:true,locked:state.locked,generation:state.generation,revision:state.revision}));
  state.quickPanelAccounts.mockReset().mockImplementation(async () => { if(state.locked)throw {code:"vault_locked"};return [account]; });
  state.codes.mockReset().mockImplementation(async () => [code()]);
  state.copy.mockReset().mockResolvedValue({token:"opaque-token"});
  state.showMain.mockReset().mockResolvedValue(undefined);
  state.settings.mockReset().mockImplementation(async () => ({locale:"zh-CN",hide_codes:state.hideCodes}));
});

describe("favorite quick panel lock and visibility lifecycle", () => {
  it("reloads favorites on focus after unlock and clears them immediately on lock", async () => {
    render(<QuickPanel />);
    expect(await screen.findByText("已锁定")).toBeVisible();
    expect(state.quickPanelAccounts).not.toHaveBeenCalled();
    await waitFor(()=>expect(state.handlers.get("vault://locked")).toBeTypeOf("function"));
    state.locked=false;
    act(()=>window.dispatchEvent(new Event("focus")));
    await waitFor(()=>expect(state.settings).toHaveBeenCalled());
    expect(state.codes).toHaveBeenCalled();
    expect(await screen.findByText("654321")).toBeVisible();
    state.locked=true; state.generation++;
    act(()=>state.handlers.get("vault://locked")?.());
    expect(screen.getByText("已锁定")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    state.locked=false;
    act(()=>window.dispatchEvent(new Event("focus")));
    expect(await screen.findByText("654321")).toBeVisible();
  });

  it.each([-3600, 3600])("keeps a backend-active TOTP usable across T0 with clock offset %i", async (offset) => {
    state.locked = false;
    state.settings.mockResolvedValue({locale:"en",hide_codes:false});
    const effectiveNow = Math.floor(Date.now()/1000) - offset;
    state.quickPanelAccounts.mockResolvedValue([{...account,t0:String(effectiveNow - 60)}]);
    state.codes.mockResolvedValue([{...code("654321",effectiveNow),clock_offset_secs:offset}]);
    render(<QuickPanel />);
    expect(await screen.findByText("654321")).toBeVisible();
    expect(screen.queryByText("Not active yet")).not.toBeInTheDocument();
    const copy = screen.getByRole("button", {name:"Copy code: Example alice"});
    expect(copy).toBeEnabled();
    fireEvent.click(copy);
    await waitFor(() => expect(state.copy).toHaveBeenCalledExactlyOnceWith(account.id,"654321"));
  });

  it("uses the saved language and updates document language", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",hide_codes:false});
    render(<QuickPanel />);
    expect(await screen.findByText("Favorites")).toBeVisible();
    expect(await screen.findByText("654321")).toBeVisible();
    expect(document.documentElement.lang).toBe("en");
  });

  it("publishes the saved light theme in the standalone document and follows saved changes", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",theme:"light",animations:false,hide_codes:false});
    render(<QuickPanel />);
    expect(await screen.findByText("654321")).toBeVisible();
    expect(document.documentElement).toHaveAttribute("data-theme","light");
    expect(screen.getByRole("main")).toHaveAttribute("data-theme","light");
    expect(document.documentElement).toHaveAttribute("data-motion","off");
    await waitFor(()=>expect(state.handlers.get("settings://changed")).toBeTypeOf("function"));
    state.settings.mockResolvedValue({locale:"en",theme:"dark",hide_codes:false});
    state.revision++;
    act(()=>state.handlers.get("settings://changed")?.());
    await waitFor(()=>expect(document.documentElement).toHaveAttribute("data-theme","dark"));
    expect(screen.getByRole("main")).toHaveAttribute("data-theme","dark");
  });

  it("uses the dark fallback before settings load and for unknown themes", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",theme:"unknown",hide_codes:false});
    render(<QuickPanel />);
    expect(document.documentElement).toHaveAttribute("data-theme","dark");
    expect(await screen.findByText("654321")).toBeVisible();
    expect(document.documentElement).toHaveAttribute("data-theme","dark");
  });

  it("distinguishes search misses from an empty favorites list", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",theme:"light",hide_codes:false});
    render(<QuickPanel />);
    await screen.findByText("654321");
    fireEvent.change(screen.getByRole("textbox"),{target:{value:"no-matching-service"}});
    expect(screen.getByText("No matching accounts")).toBeVisible();
    expect(screen.queryByText("No accounts here yet")).not.toBeInTheDocument();
  });

  it("discards a delayed code response after the vault locks", async () => {
    const pending=deferred<ReturnType<typeof code>[]>();
    state.locked=false;
    state.codes.mockImplementation(()=>pending.promise);
    render(<QuickPanel />);
    await waitFor(()=>expect(state.codes).toHaveBeenCalled());
    act(()=>state.handlers.get("vault://locked")?.());
    await act(async()=>pending.resolve([code()]));
    expect(screen.getByText("已锁定")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
  });

  it("masks immediately on settings event, drops delayed codes and makes no code IPC while hidden", async () => {
    state.locked=false;
    const pending=deferred<ReturnType<typeof code>[]>();
    let delay=false;
    state.codes.mockImplementation(()=>delay?pending.promise:Promise.resolve([code()]));
    render(<QuickPanel />);
    expect(await screen.findByText("654321")).toBeVisible();
    await waitFor(()=>expect(state.handlers.get("settings://changed")).toBeTypeOf("function"));
    delay=true;
    act(()=>window.dispatchEvent(new Event("focus")));
    await waitFor(()=>expect(state.codes).toHaveBeenCalledTimes(2));
    const count=state.codes.mock.calls.length;
    state.hideCodes=true;state.revision++;
    act(()=>state.handlers.get("settings://changed")?.());
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    expect(screen.getByText("••••••")).toBeVisible();
    expect(screen.getByRole("button",{name:"复制验证码: Example alice"})).toBeDisabled();
    await act(async()=>pending.resolve([code("999888")]));
    await waitFor(()=>expect(state.settings).toHaveBeenCalled());
    await act(async()=>window.dispatchEvent(new Event("focus")));
    expect(screen.queryByText("999888")).not.toBeInTheDocument();
    expect(state.codes).toHaveBeenCalledTimes(count);
    fireEvent.click(screen.getByRole("button",{name:"复制验证码: Example alice"}));
    expect(state.copy).not.toHaveBeenCalled();
  });

  it("shows future TOTP as not active while keeping other OTPs usable", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",hide_codes:false});
    const future={...account,id:"future",account_name:"future",t0:String(BigInt(Math.floor(Date.now()/1000))+3600n)};
    const hotp={...account,id:"counter",account_name:"counter",kind:"hotp"};
    state.quickPanelAccounts.mockResolvedValue([future,account,hotp]);
    state.codes.mockResolvedValue([
      {...code("111111"),account_id:account.id},
      {...code("222222"),account_id:hotp.id},
    ]);
    render(<QuickPanel />);
    const futureStatus = await screen.findByText("Not active yet");
    expect(futureStatus).toBeVisible();
    expect(futureStatus).toHaveClass("future-t0-status");
    const futureRow = futureStatus.closest(".quick-panel-row");
    expect(futureRow).not.toBeNull();
    expect(futureRow?.querySelector(".quick-panel-code .future-t0-status")).toBe(futureStatus);
    expect(futureRow?.querySelector(".quick-panel-code span:not(.future-t0-status)")).not.toBeInTheDocument();
    expect(screen.queryByText("0s")).not.toBeInTheDocument();
    expect(screen.queryByText("------")).not.toBeInTheDocument();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Copy code: Example future"})).toBeDisabled();
    expect(screen.getByText("111111")).toBeVisible();
    expect(screen.getByText("222222")).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Copy code: Example future"}));
    expect(state.copy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:"Copy code: Example alice"}));
    await waitFor(()=>expect(state.copy).toHaveBeenCalledWith(account.id,"111111"));
  });

  it("does not show an expired OTP or allow copying when a delayed refresh has expired", async () => {
    state.locked=false;
    state.codes.mockResolvedValue([code("654321",Math.floor(Date.now()/1000)-12)]);
    render(<QuickPanel />);
    expect(await screen.findByText("------")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"复制验证码: Example alice"})).toBeDisabled();
    fireEvent.click(screen.getByRole("button",{name:"复制验证码: Example alice"}));
    expect(state.copy).not.toHaveBeenCalled();
  });

  it("clears displayed codes and keeps a retryable error on refresh failure", async () => {
    state.locked=false;
    render(<QuickPanel />);
    expect(await screen.findByText("654321")).toBeVisible();
    state.codes.mockRejectedValue({code:"storage"});
    act(()=>window.dispatchEvent(new Event("focus")));
    expect(await screen.findByText("操作未完成")).toBeVisible();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"重试"})).toBeEnabled();
  });

  it.each(["vault://locked", "settings://changed"])("clears copy feedback on %s", async (event) => {
    state.locked=false;
    state.settings.mockImplementation(async () => ({locale:"en",hide_codes:state.hideCodes}));
    render(<QuickPanel />);
    fireEvent.click(await screen.findByRole("button", {name:"Copy code: Example alice"}));
    expect(await screen.findByText("Copied: Example · alice")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox"), {target:{value:"alice"}});
    if (event === "vault://locked") { state.locked=true; state.generation++; }
    else { state.hideCodes=true; state.revision++; }
    await act(async () => state.handlers.get(event)?.());
    expect(screen.queryByText("Copied: Example · alice")).not.toBeInTheDocument();
    expect(screen.queryByText("654321")).not.toBeInTheDocument();
    if (state.locked) expect(screen.getByRole("textbox")).toHaveValue("");
    else expect(screen.getByRole("button", {name:"Copy code: Example alice"})).toBeDisabled();
  });

  it("passes the displayed code as the guarded Copy expectation", async () => {
    state.locked=false;
    state.settings.mockResolvedValue({locale:"en",theme:"dark",hide_codes:false});
    render(<QuickPanel />);
    expect(await screen.findByText("654321")).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Copy code: Example alice"}));
    await waitFor(()=>expect(state.copy).toHaveBeenCalledWith(account.id,"654321"));
    expect(await screen.findByText("Copied: Example · alice")).toBeVisible();
    expect(screen.getByRole("button",{name:"Copied: Example alice"})).toBeVisible();
  });
});
