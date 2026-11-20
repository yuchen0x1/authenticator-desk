import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { getMessages } from "../../i18n";
import { AppShell } from "./AppShell";

it("keeps localized names and the current destination in the icon-only navigation", () => {
  const onNavigate = vi.fn();
  const props = { locked: false, onNavigate, onLock: vi.fn(), children: null };
  const { rerender } = render(<AppShell {...props} locale="en" page="accounts" />);
  const en = getMessages("en").navigation;
  const nav = screen.getByRole("navigation", { name: en.label });

  // Simulate the sidebar breakpoint that hides visual text labels.
  const buttons = within(nav).getAllByRole("button");
  for (const button of buttons) (button.lastElementChild as HTMLElement).style.display = "none";
  const { label: _label, ...destinations } = en;
  for (const name of Object.values(destinations)) {
    expect(within(nav).getByRole("button", { name })).toHaveAttribute("aria-label", name);
  }
  expect(within(nav).getByRole("button", { name: en.accounts })).toHaveAttribute("aria-current", "page");
  fireEvent.click(within(nav).getByRole("button", { name: en.settings }));
  expect(onNavigate).toHaveBeenCalledExactlyOnceWith("settings");

  rerender(<AppShell {...props} locale="ja" page="settings" />);
  const ja = getMessages("ja").navigation;
  const translatedNav = screen.getByRole("navigation", { name: ja.label });
  expect(within(translatedNav).getByRole("button", { name: ja.settings })).toHaveAttribute("aria-current", "page");
  expect(within(translatedNav).getByRole("button", { name: ja.accounts })).not.toHaveAttribute("aria-current");
});
