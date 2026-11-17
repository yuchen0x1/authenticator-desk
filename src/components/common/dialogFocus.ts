import type { KeyboardEvent } from "react";

export function trapDialogFocus(event:KeyboardEvent<HTMLElement>,close:()=>void) {
  if(event.key==="Escape"){event.preventDefault();close();return;}
  if(event.key!=="Tab")return;
  const items=[...event.currentTarget.querySelectorAll<HTMLElement>("button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex='0']")];
  const first=items[0],last=items.at(-1);
  if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}
  else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}
}
