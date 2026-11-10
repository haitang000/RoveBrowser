import { useBrowser } from "../stores/browser";
import { SHORTCUTS, sameShortcut, shortcutFor, shortcutFromEvent } from "./settings";
export function browserKey(b: ReturnType<typeof useBrowser>, e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return;
  if (e.key === "Escape" && b.findOpen) {
    e.preventDefault();
    b.closeFind();
    return;
  }
  if (e.ctrlKey && !e.altKey && /^[1-9]$/.test(e.key)) {
    const space = b.state.spaces[Number(e.key) - 1];
    if (space) { e.preventDefault(); b.send({ type: "space", id: space.id }); }
    return;
  }
  const value = shortcutFromEvent(e);
  const action = value && SHORTCUTS.find((item) => sameShortcut(value, shortcutFor(item.id, b.settings)))?.id;
  if (!action) return;
  e.preventDefault();
  if (action === "address") b.openCommand("edit");
  else if (action === "new") b.openCommand();
  else if (action === "settings") b.openSettings();
  else if (action === "cycle" || action === "cyclePrevious") b.cycle(action === "cyclePrevious");
  else if (action === "desk") b.deskOpen = !b.deskOpen;
  else if (action === "find") {
    // A dialog owns the keyboard while it is up; otherwise ⌘F toggles, so
    // pressing it while typing in the bar closes it, as in Zigzag.
    if (b.commandOpen || b.deskOpen || b.settingsOpen) return;
    const field = document.activeElement;
    if (b.findOpen && field instanceof HTMLInputElement && field.closest(".find-bar")) b.closeFind();
    else b.openFind();
  } else b.send({ type: action });
}
