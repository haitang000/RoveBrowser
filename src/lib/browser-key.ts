import { useBrowser } from "../stores/browser";
import { SHORTCUTS, sameShortcut, shortcutFor, shortcutFromEvent } from "./settings";
export function browserKey(b: ReturnType<typeof useBrowser>, e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return;
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
  else b.send({ type: action });
}
