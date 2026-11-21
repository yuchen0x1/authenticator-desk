import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { defaultSettings, type WorkspaceProps } from "../App";
import type { AccountView } from "../lib/types";
import { DashboardPage } from "./DashboardPage";

const dashboard = vi.hoisted(()=>vi.fn());
vi.mock("../lib/tauri",()=>({activityCommands:{dashboard}}));
const account: AccountView={id:"demo",issuer:"Synthetic service",account_name:"test@example.invalid",algorithm:"SHA1",digits:6,kind:"totp",period_secs:30,t0:"0",hotp_counter:"0",category_id:null,tags:[],notes_present:false,favorite:true,sort_order:0,created_at:"",updated_at:""};
const summary=(accounts:AccountView[])=>({account_count:accounts.length,category_count:0,favorite_count:accounts.filter(a=>a.favorite).length,copies_today:0,recent_accounts:accounts});
function workspace(): WorkspaceProps {
  return {locale:"en",accounts:[],categories:[],codes:[],settings:defaultSettings,refresh:vi.fn(async()=>{}),notify:vi.fn(),onError:vi.fn(),onAdd:vi.fn(),onEdit:vi.fn(),navigate:vi.fn(),saveSettings:vi.fn(async()=>{})};
}
describe("dashboard refresh",()=>{
  it("refreshes summary and recent accounts after an account mutation without navigation",async()=>{
    dashboard.mockReset();dashboard.mockResolvedValueOnce(summary([])).mockResolvedValueOnce(summary([account]));
    const props=workspace();
    const view=render(<DashboardPage {...props}/>);
    await waitFor(()=>expect(dashboard).toHaveBeenCalledTimes(1));
    view.rerender(<DashboardPage {...props} accounts={[account]}/>);
    expect(await screen.findByText("Synthetic service")).toBeVisible();
    expect(dashboard).toHaveBeenCalledTimes(2);
  });
  it("ignores a stale summary response after newer account data arrives",async()=>{
    let finish:((value:ReturnType<typeof summary>)=>void)|undefined;
    dashboard.mockReset();dashboard.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve})).mockResolvedValueOnce(summary([account]));
    const props=workspace();
    const view=render(<DashboardPage {...props}/>);
    await waitFor(()=>expect(finish).toBeDefined());
    view.rerender(<DashboardPage {...props} accounts={[account]}/>);
    await screen.findByText("Synthetic service");
    await act(async()=>finish?.(summary([])));
    expect(screen.getByText("Synthetic service")).toBeVisible();
  });
});
