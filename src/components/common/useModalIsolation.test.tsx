import { render, screen } from "@testing-library/react";
import { StrictMode, useRef } from "react";
import { createPortal } from "react-dom";
import { describe, expect, it } from "vitest";
import { useModalIsolation } from "./useModalIsolation";

function Modal({ name, open }: { name: string; open: boolean }) {
  const dialog = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  useModalIsolation(open, dialog, returnFocus);
  return open ? createPortal(<div><div ref={dialog} role="dialog" aria-label={name} /></div>, document.body) : null;
}

describe("useModalIsolation", () => {
  it.each(["top", "bottom"])("retains isolation when nested modals close %s-first, including StrictMode cleanup", (order) => {
    function Modals({ first, second }: { first: boolean; second: boolean }) {
      return <StrictMode><Modal name="First" open={first} /><Modal name="Second" open={second} /></StrictMode>;
    }
    const { container, rerender, unmount } = render(<Modals first second />);
    expect(container).toHaveAttribute("inert");
    expect(screen.getByRole("dialog", { name: "First" }).closest("[inert]")).not.toBeNull();
    expect(screen.getByRole("dialog", { name: "Second" }).closest("[inert]")).toBeNull();

    const firstRemains = order === "top";
    rerender(<Modals first={firstRemains} second={!firstRemains} />);
    expect(container).toHaveAttribute("inert");
    expect(screen.getByRole("dialog").closest("[inert]")).toBeNull();
    unmount();
    expect(container).not.toHaveAttribute("inert");
  });
});
