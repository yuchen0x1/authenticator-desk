import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AccountView, CodeView } from "../../lib/types";
import { AccountCard } from "./AccountCard";

const account:AccountView={id:"id",issuer:"GitHub",account_name:"alice",algorithm:"SHA1",digits:6,kind:"totp",period_secs:30,t0:"0",hotp_counter:"0",category_id:null,tags:[],notes_present:false,favorite:false,sort_order:0,created_at:"",updated_at:""};
const code:CodeView={account_id:"id",code:"123456",remaining_secs:12,progress:.6,generated_at:59,clock_offset_secs:0,wall_clock_jump:false};
const labels={copy:"Copy",edit:"Edit",favorite:"Favorite",unfavorite:"Unfavorite",select:"Select",hotp:"Next counter"};
describe("account code rendering",()=>{
  it("removes the accessible code when masked and disables copying",()=>{
    render(<AccountCard account={account} code={code} masked selected={false} copyAvailable t={labels} onSelect={()=>{}} onEdit={()=>{}} onFavorite={()=>{}} onCopy={()=>{}} onAdvance={()=>{}}/>);
    expect(screen.queryByLabelText("123456")).not.toBeInTheDocument();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Copy"})).toBeDisabled();
  });
  it("calls selected account actions without exposing its secret",()=>{
    const onFavorite=vi.fn();const onCopy=vi.fn();
    render(<AccountCard account={account} code={code} masked={false} selected={false} copyAvailable t={labels} onSelect={()=>{}} onEdit={()=>{}} onFavorite={onFavorite} onCopy={onCopy} onAdvance={()=>{}}/>);
    expect(screen.getByLabelText("123456")).toBeVisible();
    fireEvent.click(screen.getByRole("button",{name:"Favorite"}));
    fireEvent.click(screen.getByRole("button",{name:"Copy"}));
    expect(onFavorite).toHaveBeenCalledWith(account);
    expect(onCopy).toHaveBeenCalledWith(account);
  });
});
