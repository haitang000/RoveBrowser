import type { BrowserState, PinLayout } from "./types";

export function pinFolderById(layout?: PinLayout) {
  return new Map(layout?.folders.flatMap((folder) => folder.pins.map((id) => [id, folder.id] as const)));
}

export function pinFolderName(input: unknown) {
  const name = typeof input === "string" ? input.trim() : "";
  if (!name || Array.from(name).length > 80) throw Error("请输入 1–80 个字符的文件夹名称");
  return name;
}

export function movePinEntries(layout: PinLayout, moving: string[], folderId: string | undefined, anchor: string[], after: boolean) {
  if (anchor.some((id) => moving.includes(id))) return;
  layout.root = layout.root.filter((id) => !moving.includes(id));
  for (const folder of layout.folders) folder.pins = folder.pins.filter((id) => !moving.includes(id));
  const entries = folderId ? layout.folders.find((folder) => folder.id === folderId)!.pins : layout.root;
  const matches = entries.map((id) => anchor.includes(id));
  const at = after ? matches.lastIndexOf(true) : matches.indexOf(true);
  entries.splice(at < 0 ? entries.length : at + Number(after), 0, ...moving);
}

/** Mirror the native layout repair for the standalone UI preview. */
export function syncPinLayouts(state: BrowserState) {
  for (const [spaceId, layout] of Object.entries(state.pinLayouts ?? {})) {
    if (!state.spaces.some((space) => space.id === spaceId)) {
      delete state.pinLayouts![spaceId];
      continue;
    }
    const pins = state.pins!.filter((pin) => pin.spaceId === spaceId);
    const byId = new Map(pins.map((pin) => [pin.id, pin]));
    const folders = new Map(layout.folders.map((folder) => [folder.id, folder]));
    const seen = new Set<string>();
    function addPin(id: string) {
      const pin = byId.get(id);
      if (!pin || seen.has(id)) return [];
      seen.add(id);
      const other = pin.splitWith;
      if (other && byId.has(other) && !seen.has(other)) {
        seen.add(other);
        return [id, other];
      }
      return [id];
    }
    for (const folder of layout.folders) folder.pins = folder.pins.flatMap(addPin);
    const placed = new Set<string>();
    layout.root = [...layout.root, ...folders.keys()].flatMap((id) => {
      if (!folders.has(id)) return addPin(id);
      if (placed.has(id)) return [];
      placed.add(id);
      return [id];
    });
    layout.root.push(...pins.flatMap((pin) => addPin(pin.id)));
    const ordered = layout.root.flatMap((id) => folders.get(id)?.pins ?? [id]);
    let index = 0;
    state.pins = state.pins!.map((pin) => pin.spaceId === spaceId ? byId.get(ordered[index++])! : pin);
    if (!layout.folders.length) delete state.pinLayouts![spaceId];
  }
}
