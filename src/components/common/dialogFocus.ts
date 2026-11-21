import type { KeyboardEvent } from "react";

function focusableItems(dialog: HTMLElement) {
  return [...dialog.querySelectorAll<HTMLElement>("button,input,select,textarea,a[href],[tabindex],[contenteditable='true']")]
    .filter((item) => {
      // :disabled also covers controls disabled by an ancestor fieldset.
      if (item.matches(":disabled,input[type='hidden']") || item.tabIndex < 0 || item.closest("[hidden],[inert],[aria-hidden='true']")) return false;
      for (let node: HTMLElement | null = item; node; node = node.parentElement) {
        const style = getComputedStyle(node);
        if (style.display === "none" || style.visibility === "hidden") return false;
        if (node === dialog) break;
      }
      return true;
    });
}

export function focusDialog(dialog: HTMLElement) {
  const first = focusableItems(dialog)[0];
  if (first) first.focus();
  else {
    if (!dialog.hasAttribute("tabindex")) dialog.tabIndex = -1;
    dialog.focus();
  }
}

export function trapDialogFocus(event: KeyboardEvent<HTMLElement>, close: () => void) {
  if (event.key === "Escape") { event.preventDefault(); close(); return; }
  if (event.key !== "Tab") return;
  const dialog = event.currentTarget;
  const items = focusableItems(dialog);
  const first = items[0], last = items.at(-1);
  const active = document.activeElement;
  if (!items.some((item) => item === active)) {
    event.preventDefault();
    if (items.length) (event.shiftKey ? last : first)?.focus();
    else focusDialog(dialog);
  } else if (event.shiftKey && active === first) {
    event.preventDefault(); last?.focus();
  } else if (!event.shiftKey && active === last) {
    event.preventDefault(); first?.focus();
  }
}
