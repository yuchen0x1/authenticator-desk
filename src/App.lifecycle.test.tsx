import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App, { defaultSettings } from "./App";

const ipc = vi.hoisted(() => ({ invoke: vi.fn(), locked: undefined as (() => void) | undefined }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: ipc.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (_event: string, handler: () => void) => { ipc.locked = handler; return () => { ipc.locked = undefined; }; }) }));
const account = { id:"account-one",issuer:"GitHub",account_name:"alice",algorithm:"SHA1",digits:6,kind:"totp",period_secs:30,t0:"0",hotp_counter:"0",category_id:null,tags:[],notes_present:false,favorite:false,sort_order:0,created_at:"2026-11-17T00:00:00Z",updated_at:"2026-11-17T00:00:00Z" };
beforeEach(() => {
  ipc.invoke.mockReset();
  ipc.invoke.mockImplementation(async (command: string) => {
    if(command === "app_snapshot") return {locked:false,vault_exists:true,locale:"en",theme:"dark"};
    if(command === "settings") return {...defaultSettings,locale:"en"};
    if(command === "list_accounts") return [account];
    if(command === "list_categories") return [];
    if(command === "vault_status") return {exists:true,locked:false,generation:1,revision:1};
    if(command === "generate_codes") return [];
    if(command === "dashboard") return {account_count:1,category_count:0,favorite_count:0,copies_today:0,recent_accounts:[account]};
    return undefined;
  });
});

describe("vault UI boundary", () => {
  it("removes accounts immediately and rejects late code results after a lock event", async () => {
    let resolveCodes: ((value: unknown) => void) | undefined;
    ipc.invoke.mockImplementation(async (command: string, payload: {ids?:string[]}) => {
      if(command === "app_snapshot") return {locked:false,vault_exists:true,locale:"en",theme:"dark"};
      if(command === "settings") return {...defaultSettings,locale:"en"};
      if(command === "list_accounts") return [account];
      if(command === "list_categories") return [];
      if(command === "vault_status") return {exists:true,locked:false,generation:1,revision:1};
      if(command === "generate_codes" && payload?.ids?.length) return new Promise((resolve)=>{resolveCodes=resolve});
      if(command === "generate_codes") return [];
      if(command === "dashboard") return {account_count:1,category_count:0,favorite_count:0,copies_today:0,recent_accounts:[account]};
      return undefined;
    });
    render(<App />);
    await screen.findByRole("button", {name:/Add account/});
    fireEvent.click(screen.getByRole("button",{name:"Codes"}));
    expect(await screen.findByText("alice")).toBeVisible();
    await waitFor(()=>expect(resolveCodes).toBeDefined(), {timeout:2500});
    act(()=>ipc.locked?.());
    expect(screen.queryByText("alice")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Unlock vault"})).toBeVisible();
    await act(async()=>resolveCodes?.([{account_id:account.id,code:"123456",remaining_secs:12,progress:.6,generated_at:59,clock_offset_secs:0,wall_clock_jump:false}]));
    expect(screen.queryByLabelText("123456")).not.toBeInTheDocument();
  });

  it("changes the runtime language only after saving real settings", async () => {
    render(<App />);
    await screen.findByRole("button",{name:/Add account/});
    fireEvent.click(screen.getByRole("button",{name:"Settings"}));
    fireEvent.change(screen.getByLabelText("Language"),{target:{value:"ja"}});
    fireEvent.click(screen.getByRole("button",{name:"Save"}));
    expect(await screen.findByRole("heading",{name:"環境設定"})).toBeVisible();
    expect(ipc.invoke).toHaveBeenCalledWith("save_settings",{settings:expect.objectContaining({locale:"ja"})});
  });
});
