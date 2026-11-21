import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { trapDialogFocus } from "./dialogFocus";

describe("trapDialogFocus", () => {
  it("skips inherited disabled, hidden, inert and negative-tabindex controls in both directions", () => {
    render(<div role="dialog" tabIndex={-1} onKeyDown={(event) => trapDialogFocus(event, vi.fn())}>
      <button>First</button>
      <fieldset disabled><input aria-label="Inherited disabled" /><button tabIndex={0}>Disabled child</button></fieldset>
      <input type="hidden" aria-label="Hidden input" />
      <div hidden><button>Hidden ancestor</button></div>
      <div inert><button>Inert ancestor</button></div>
      <button style={{ display: "none" }}>CSS hidden</button>
      <button tabIndex={-1}>Not tabbable</button>
      <a href="#last">Last</a>
    </div>);
    const first = screen.getByRole("button", { name: "First" });
    const last = screen.getByRole("link", { name: "Last" });
    first.focus();
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(last).toHaveFocus();
    fireEvent.keyDown(last, { key: "Tab" });
    expect(first).toHaveFocus();
  });

  it("keeps Tab on the panel when every field is disabled and recovers focus when controls return", () => {
    const close = vi.fn();
    function Dialog({ busy }: { busy: boolean }) {
      return <div role="dialog" tabIndex={-1} onKeyDown={(event) => trapDialogFocus(event, close)}>
        <fieldset disabled={busy}><input aria-label="Field" /><button>Save</button></fieldset>
      </div>;
    }
    const { rerender } = render(<Dialog busy />);
    const dialog = screen.getByRole("dialog");
    dialog.focus();
    expect(fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true })).toBe(false);
    expect(dialog).toHaveFocus();
    rerender(<Dialog busy={false} />);
    fireEvent.keyDown(dialog, { key: "Tab" });
    expect(screen.getByLabelText("Field")).toHaveFocus();
    dialog.focus();
    fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
    expect(screen.getByRole("button", { name: "Save" })).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
  });
});
