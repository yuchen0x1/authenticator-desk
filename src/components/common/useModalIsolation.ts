import { useLayoutEffect, type RefObject } from "react";

const modals: HTMLElement[] = [];
const previousInert = new Map<Element, string | null>();
let observer: MutationObserver | undefined;

function restoreInert(element: Element, value: string | null) {
  if (value === null) element.removeAttribute("inert");
  else element.setAttribute("inert", value);
}

function isolateTopModal() {
  const modal = modals.at(-1);
  const body = modal?.ownerDocument.body;
  for (const [element, value] of previousInert) {
    if (!body || element.parentElement !== body || element.contains(modal!)) {
      restoreInert(element, value);
      previousInert.delete(element);
    }
  }
  if (!body) return;
  for (const element of body.children) {
    if (element.contains(modal!)) continue;
    if (!previousInert.has(element)) previousInert.set(element, element.getAttribute("inert"));
    element.setAttribute("inert", "");
  }
}

// Isolate body portals as well as the app root, retaining any pre-existing inert
// attributes. A stack keeps cleanup safe if another modal is mounted on top.
export function useModalIsolation(open: boolean, dialog: RefObject<HTMLElement | null>, returnFocus?: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    const modal = dialog.current;
    if (!open || !modal) return;
    const focusTarget = returnFocus?.current ?? (modal.ownerDocument.activeElement instanceof HTMLElement ? modal.ownerDocument.activeElement : null);
    modals.push(modal);
    isolateTopModal();
    if (!observer) {
      observer = new MutationObserver(isolateTopModal);
      observer.observe(modal.ownerDocument.body, { childList: true });
    }
    return () => {
      const wasTop = modals.at(-1) === modal;
      const index = modals.lastIndexOf(modal);
      if (index !== -1) modals.splice(index, 1);
      if (!modals.length) {
        observer?.disconnect();
        observer = undefined;
      }
      isolateTopModal();
      // Restore focus only after background controls stop being inert.
      if (wasTop && focusTarget?.isConnected && !focusTarget.closest("[inert]")) focusTarget.focus();
    };
  }, [open, dialog, returnFocus]);
}
