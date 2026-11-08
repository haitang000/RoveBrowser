import { useBrowser } from "../stores/browser";
export function browserKey(b: ReturnType<typeof useBrowser>, e: KeyboardEvent) {
  const k = e.key.toLowerCase();
  if (e.ctrlKey && !e.altKey) {
    if (k === "l") {
      e.preventDefault();
      b.openCommand("edit");
    }
    if (k === "t") {
      e.preventDefault();
      if (e.shiftKey) b.send({ type: "restore" });
      else b.openCommand();
    }
    if (k === "w") {
      e.preventDefault();
      b.send({ type: "close" });
    }
    if (k === "tab") {
      e.preventDefault();
      b.cycle(e.shiftKey);
    }
    if (k === "b") {
      e.preventDefault();
      b.send({ type: "sidebar" });
    }
    if (k === "r") {
      e.preventDefault();
      b.send({ type: "reload" });
    }
    if (e.code === "Backslash" && e.shiftKey) {
      e.preventDefault();
      b.deskOpen = !b.deskOpen;
    }
    if (/^[1-9]$/.test(k)) {
      const s = b.state.spaces[Number(k) - 1];
      if (s) {
        e.preventDefault();
        b.send({ type: "space", id: s.id });
      }
    }
  }
  if (e.altKey && (k === "arrowleft" || k === "arrowright")) {
    e.preventDefault();
    b.send({ type: k === "arrowleft" ? "back" : "forward" });
  }
}
