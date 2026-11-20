import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AccountView, CodeView } from "../../lib/types";
import { AccountCard } from "./AccountCard";

const account: AccountView = { id: "id", issuer: "GitHub", account_name: "alice", algorithm: "SHA1", digits: 6, kind: "totp", period_secs: 30, t0: "0", hotp_counter: "0", category_id: null, tags: [], notes_present: false, favorite: false, sort_order: 0, created_at: "", updated_at: "" };
const code: CodeView = { account_id: "id", code: "123456", remaining_secs: 12, progress: .6, generated_at: 59, clock_offset_secs: 0, wall_clock_jump: false };
const labels = { copy: "Copy", edit: "Edit", favorite: "Favorite", unfavorite: "Unfavorite", select: "Select", hotp: "HOTP", advanceHotp: "Next counter", hiddenCode: "Verification code hidden", futureT0: "Not active yet" };
const actionName = (action: string, name = "alice") => `${action}: GitHub · ${name}`;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-11-20T00:00:59Z"));
  code.generated_at = Math.floor(Date.now() / 1000);
});
afterEach(() => vi.useRealTimers());

function cardProps() {
  return { account, code: { ...code, generated_at: Math.floor(Date.now() / 1000) }, masked: false, selected: false, copyAvailable: true, t: labels, onSelect: vi.fn(), onEdit: vi.fn(), onFavorite: vi.fn(), onCopy: vi.fn(), onAdvance: vi.fn() };
}

describe("account code rendering", () => {
  it("removes the accessible code when masked and disables copying", () => {
    const props = cardProps();
    const { container } = render(<AccountCard {...props} masked />);
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByText(labels.hiddenCode)).toBeInTheDocument();
    expect(container.innerHTML).not.toContain("123456");
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeDisabled();
    expect(screen.getByRole("img", { name: "0 s" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    expect(props.onCopy).not.toHaveBeenCalled();
  });

  it("calls selected account actions without exposing its secret", () => {
    const props = cardProps();
    render(<AccountCard {...props} />);
    expect(screen.getByText("123 456")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.favorite) }));
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.edit) }));
    fireEvent.click(screen.getByRole("checkbox", { name: actionName(labels.select) }));
    expect(props.onFavorite).toHaveBeenCalledWith(account);
    expect(props.onCopy).toHaveBeenCalledWith(account);
    expect(props.onEdit).toHaveBeenCalledWith(account);
    expect(props.onSelect).toHaveBeenCalledWith(account.id, true);
  });

  it("disambiguates repeated services by account name in every action", () => {
    const props = cardProps();
    const bob = { ...account, id: "bob", account_name: "bob", favorite: true };
    render(<><AccountCard {...props} /><AccountCard {...props} account={bob} /></>);
    const row = screen.getByRole("listitem", { name: "GitHub · bob" });
    expect(within(row).getByText("bob")).toHaveAttribute("title", "bob");
    expect(within(row).getByRole("button", { name: actionName(labels.unfavorite, "bob") })).toBeEnabled();
    fireEvent.click(within(row).getByRole("button", { name: actionName(labels.copy, "bob") }));
    expect(props.onCopy).toHaveBeenCalledExactlyOnceWith(bob);
  });

  it("expires the code and Copy without a fresh response or parent rerender", () => {
    const props = cardProps();
    const { rerender } = render(<AccountCard {...props} />);
    expect(screen.getByText("123 456")).toBeVisible();
    expect(screen.getByRole("img", { name: "12 s" })).toBeVisible();
    act(() => vi.advanceTimersByTime(12000));
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByText("------")).toBeVisible();
    expect(screen.getByRole("img", { name: "0 s" })).toBeVisible();
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    expect(props.onCopy).not.toHaveBeenCalled();
    rerender(<AccountCard {...props} code={{ ...code, code: "654321", generated_at: Math.floor(Date.now() / 1000) }} />);
    expect(screen.getByText("654 321")).toBeVisible();
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeEnabled();
  });

  it("shows a localized future-T0 status and disables Copy instead of showing a placeholder", () => {
    const future = { ...account, t0: String(Math.floor(Date.now() / 1000) + 3600) };
    const props = { ...cardProps(), account: future };
    const { container } = render(<AccountCard {...props} />);
    expect(screen.getByText("Not active yet")).toBeVisible();
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.queryByText("------")).not.toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.querySelector(".code-ring")).not.toBeInTheDocument();
    expect(screen.queryByText("0 s")).not.toBeInTheDocument();
    const copyButton = screen.getByRole("button", { name: actionName(labels.copy) });
    expect(copyButton).toBeDisabled();
    fireEvent.click(copyButton);
    expect(props.onCopy).not.toHaveBeenCalled();
  });

  it("never initially renders a code that expired while its IPC response was delayed", () => {
    render(<AccountCard {...cardProps()} code={{ ...code, generated_at: Math.floor(Date.now() / 1000) - 12 }} />);
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeDisabled();
  });

  it("rechecks expiry on click even if the clock timer has not run", () => {
    const props = cardProps();
    render(<AccountCard {...props} />);
    vi.setSystemTime(Date.now() + 12000);
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    expect(props.onCopy).not.toHaveBeenCalled();
  });

  it("uses the backend compensated clock when the system wall clock jumped", () => {
    const props = cardProps();
    const offset = 3600;
    render(<AccountCard {...props} code={{ ...props.code, generated_at: props.code.generated_at - offset, clock_offset_secs: offset }} />);
    expect(screen.getByText("123 456")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    expect(props.onCopy).toHaveBeenCalledWith(account);
    act(() => vi.advanceTimersByTime(12000));
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
  });

  it("does not treat HOTP remaining_secs=0 as expired, but blocks advancement while hidden", () => {
    const props = { ...cardProps(), account: { ...account, kind: "hotp" as const }, code: { ...code, remaining_secs: 0 } };
    const { rerender } = render(<AccountCard {...props} />);
    act(() => vi.advanceTimersByTime(120000));
    expect(screen.getByText("123 456")).toBeVisible();
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.advanceHotp) }));
    expect(props.onAdvance).toHaveBeenCalledExactlyOnceWith(props.account);
    props.onAdvance.mockClear();
    rerender(<AccountCard {...props} masked />);
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: actionName(labels.copy) })).toBeDisabled();
    expect(screen.getByRole("button", { name: actionName(labels.advanceHotp) })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.advanceHotp) }));
    expect(props.onAdvance).not.toHaveBeenCalled();
  });

  it("immediately removes the HOTP code and disables both code actions while advancing", () => {
    const props = { ...cardProps(), account: { ...account, kind: "hotp" as const } };
    const { rerender } = render(<AccountCard {...props} />);
    rerender(<AccountCard {...props} advancing />);
    expect(screen.queryByText("123 456")).not.toBeInTheDocument();
    expect(screen.getByText("------")).toBeVisible();
    const advance = screen.getByRole("button", { name: actionName(labels.advanceHotp) });
    const copy = screen.getByRole("button", { name: actionName(labels.copy) });
    expect(advance).toBeDisabled();
    expect(advance).toHaveAttribute("aria-busy", "true");
    expect(copy).toBeDisabled();
    fireEvent.click(advance);
    fireEvent.click(copy);
    expect(props.onAdvance).not.toHaveBeenCalled();
    expect(props.onCopy).not.toHaveBeenCalled();
  });

  it("never copies without the backend copy capability or a code", () => {
    const props = cardProps();
    const { rerender } = render(<AccountCard {...props} copyAvailable={false} />);
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    rerender(<AccountCard {...props} code={undefined} />);
    fireEvent.click(screen.getByRole("button", { name: actionName(labels.copy) }));
    expect(props.onCopy).not.toHaveBeenCalled();
  });

  it("keeps the countdown ring progress within its track", () => {
    const props = cardProps();
    const { container } = render(<AccountCard {...props} code={{ ...props.code, remaining_secs: 90 }} />);
    const ring = container.querySelector(".code-ring-svg circle:last-child");
    expect(ring).toHaveAttribute("stroke-dashoffset", "0");
  });
});
