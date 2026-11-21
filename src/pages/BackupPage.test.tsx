import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BackupPage } from "./BackupPage";
import { defaultSettings, type WorkspaceProps } from "../App";

const commands = vi.hoisted(() => ({ create:vi.fn(),preview:vi.fn(),restore:vi.fn(),cancel:vi.fn(),list:vi.fn() }));
vi.mock("../lib/backup",()=>({backupCommands:commands}));
vi.mock("../components/backup/AutoBackupSettings",()=>({AutoBackupSettings:()=>null}));
const preview = {preview_id:"restore-one",mode:"replace",existing_count:7,account_count:1,category_count:0,items:[{index:0,issuer:"Demo",account_name:"synthetic",status:"new"}]};
let props: WorkspaceProps;
beforeEach(()=>{
  Object.values(commands).forEach(mock=>mock.mockReset());
  commands.preview.mockResolvedValue(preview);
  commands.cancel.mockResolvedValue(undefined);
  commands.restore.mockResolvedValue({restored_accounts:1,restored_categories:0,mode:"replace"});
  props={locale:"en",accounts:[],categories:[],codes:[],settings:defaultSettings,refresh:vi.fn(async()=>{}),notify:vi.fn(),onError:vi.fn(),onAdd:vi.fn(),onEdit:vi.fn(),navigate:vi.fn(),saveSettings:vi.fn(async()=>{})};
});
async function inspect(expectedAccount="synthetic"){
  fireEvent.click(screen.getByRole("button",{name:"Restore backup"}));
  fireEvent.change(screen.getByRole("combobox"),{target:{value:"replace"}});
  const password = document.querySelector<HTMLInputElement>('input[type="password"]')!;
  fireEvent.change(password,{target:{value:"synthetic-backup-password"}});
  fireEvent.click(screen.getByRole("button",{name:"Choose backup and preview"}));
  await screen.findByText(expectedAccount);
}
describe("backup restore review",()=>{
  it("shows removal/restoration counts and requires explicit replacement confirmation",async()=>{
    render(<BackupPage {...props}/>);
    await inspect();
    expect(screen.getByText(/Current accounts to remove/)).toHaveTextContent("7");
    expect(screen.getByText(/Selected accounts to restore/)).toHaveTextContent("1");
    expect(screen.getByText(/Settings will be replaced/)).toBeVisible();
    const restore=screen.getByRole("button",{name:"Restore selected accounts (1)"});
    expect(restore).toBeDisabled();
    expect(commands.restore).not.toHaveBeenCalled();
    const confirm=screen.getByRole("checkbox",{name:"I confirm replacing this vault"});
    expect(confirm).toHaveAttribute("aria-describedby","backup-replace-warning");
    fireEvent.click(confirm);
    fireEvent.click(restore);
    await waitFor(()=>expect(commands.restore).toHaveBeenCalledWith("restore-one",[0],true));
    expect(props.refresh).not.toHaveBeenCalled();
  });

  it("keeps a successful merge restore successful when refresh fails",async()=>{
    const refreshError=new Error("refresh failed");
    props.refresh=vi.fn().mockRejectedValueOnce(refreshError);
    commands.preview.mockResolvedValue({...preview,mode:"merge"});
    commands.restore.mockResolvedValue({restored_accounts:1,restored_categories:0,mode:"merge"});
    render(<BackupPage {...props}/>);
    fireEvent.click(screen.getByRole("button",{name:"Restore backup"}));
    fireEvent.change(document.querySelector<HTMLInputElement>('input[type="password"]')!,{target:{value:"synthetic-backup-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Choose backup and preview"}));
    await screen.findByText("synthetic");
    fireEvent.click(screen.getByRole("button",{name:"Restore selected accounts (1)"}));
    await waitFor(()=>expect(props.notify).toHaveBeenCalledExactlyOnceWith("Accounts restored: 1"));
    expect(props.refresh).toHaveBeenCalledOnce();
    expect(props.onError).toHaveBeenCalledExactlyOnceWith(refreshError);
    expect(screen.queryByText("synthetic")).not.toBeInTheDocument();
  });

  it("cancels the retained backend preview when the review is dismissed",async()=>{
    const view=render(<BackupPage {...props}/>);
    await inspect();
    fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
    await waitFor(()=>expect(commands.cancel).toHaveBeenCalledWith("restore-one"));
    expect(commands.restore).not.toHaveBeenCalled();
    view.unmount();
    expect(commands.cancel).toHaveBeenCalledTimes(1);
  });

  it("cancels a late native preview after the page unmounts",async()=>{
    let resolvePreview:((value:typeof preview)=>void)|undefined;
    commands.preview.mockImplementation(()=>new Promise(resolve=>{resolvePreview=resolve;}));
    const view=render(<BackupPage {...props}/>);
    fireEvent.click(screen.getByRole("button",{name:"Restore backup"}));
    fireEvent.change(document.querySelector<HTMLInputElement>('input[type="password"]')!,{target:{value:"synthetic-password"}});
    fireEvent.click(screen.getByRole("button",{name:"Choose backup and preview"}));
    await waitFor(()=>expect(resolvePreview).toBeDefined());
    view.unmount();
    await act(async()=>{resolvePreview?.(preview);});
    expect(commands.cancel).toHaveBeenCalledWith("restore-one");
  });

  it("retains the old review when the native picker is cancelled",async()=>{
    commands.preview.mockResolvedValueOnce(preview).mockResolvedValueOnce(null);
    render(<BackupPage {...props}/>);
    await inspect();
    const submit=screen.getByRole("button",{name:"Choose backup and preview"});
    await waitFor(()=>expect(submit).toBeEnabled());
    fireEvent.change(document.querySelector<HTMLInputElement>('input[type="password"]')!,{target:{value:"synthetic-backup-password"}});
    fireEvent.click(submit);
    await waitFor(()=>expect(commands.preview).toHaveBeenCalledTimes(2));
    await waitFor(()=>expect(submit).toBeEnabled());
    expect(screen.getByText("synthetic")).toBeVisible();
    expect(commands.cancel).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(submit);
  });

  it("clears the old review when replacing the preview fails",async()=>{
    const error={code:"invalid_input"};
    commands.preview.mockResolvedValueOnce(preview).mockRejectedValueOnce(error);
    const page={...props,onError:vi.fn()};
    render(<BackupPage {...page}/>);
    await inspect();
    const submit=screen.getByRole("button",{name:"Choose backup and preview"});
    await waitFor(()=>expect(submit).toBeEnabled());
    fireEvent.change(document.querySelector<HTMLInputElement>('input[type="password"]')!,{target:{value:"synthetic-backup-password"}});
    fireEvent.click(submit);
    await waitFor(()=>expect(commands.preview).toHaveBeenCalledTimes(2));
    await waitFor(()=>expect(page.onError).toHaveBeenCalledExactlyOnceWith(error));
    expect(screen.queryByText("synthetic")).not.toBeInTheDocument();
    expect(commands.cancel).toHaveBeenCalledWith("restore-one");
  });

  it("clears the retained review and its selected rows when the restore mode changes",async()=>{
    render(<BackupPage {...props}/>);
    await inspect();
    const mode=screen.getByRole("combobox");
    fireEvent.change(mode,{target:{value:"merge"}});
    expect(screen.queryByText("synthetic")).not.toBeInTheDocument();
    await waitFor(()=>expect(commands.cancel).toHaveBeenCalledWith("restore-one"));
  });

  it("uses action-specific busy text and locks backup fields while the picker is open",async()=>{
    let resolvePreview!: (value:typeof preview)=>void;
    commands.preview.mockImplementation(()=>new Promise(resolve=>{resolvePreview=resolve;}));
    render(<BackupPage {...props}/>);
    fireEvent.click(screen.getByRole("button",{name:"Restore backup"}));
    const password=document.querySelector<HTMLInputElement>('input[type="password"]')!;
    fireEvent.change(password,{target:{value:"synthetic-password"}});
    const submit=screen.getByRole("button",{name:"Choose backup and preview"});
    fireEvent.click(submit);
    expect(screen.getByRole("button",{name:"Opening backup picker…"})).toBeDisabled();
    expect(password).toBeDisabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
    await act(async()=>resolvePreview(preview));
    expect(await screen.findByRole("region",{name:"Restore backup"})).toHaveFocus();
  });

  it("truncates long restore account names and returns focus after dismissing the review",async()=>{
    const longName="account-"+"x".repeat(180);
    commands.preview.mockResolvedValue({...preview,items:[{index:0,issuer:"Demo",account_name:longName,status:"new"}]});
    render(<BackupPage {...props}/>);
    await inspect(longName);
    expect(screen.getByText(longName)).toHaveClass("truncate");
    const submit=screen.getByRole("button",{name:"Choose backup and preview"});
    fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
    expect(document.activeElement).toBe(submit);
  });
});
