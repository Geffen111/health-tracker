// Where to draw a row's 3-dot menu.
//
// The list cards clip their contents (overflow:hidden keeps row backgrounds inside the
// rounded corners), so a menu positioned inside the card was cut off on the last rows.
// The menu is drawn `position: fixed` instead, at the trigger's on-screen position,
// and opens upward when there isn't room below it in the window.

export interface MenuPos { top: number; right: number; up: boolean; }

const ROOM_NEEDED = 170; // px — a three-item menu plus a margin

export function menuPosFor(trigger: HTMLElement): MenuPos {
  const r = trigger.getBoundingClientRect();
  const up = window.innerHeight - r.bottom < ROOM_NEEDED && r.top > ROOM_NEEDED;
  return { top: up ? r.top - 6 : r.bottom + 6, right: window.innerWidth - r.right, up };
}

/** Inline style for the menu element. */
export function menuStyle(p: MenuPos): string {
  return `top:${p.top}px;right:${p.right}px;${p.up ? 'transform:translateY(-100%);' : ''}`;
}
