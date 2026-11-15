import { toRaw } from "vue";

export interface StatePatch {
  type: "statePatch";
  fields: Record<string, unknown>;
  removed: string[];
  lists: Record<string, { upsert: { id: string }[]; order?: string[] }>;
}

export function expandSnapshot<T extends object>(current: T, patch: StatePatch): T {
  const next = { ...current, ...patch.fields } as Record<string, unknown>;
  for (const key of patch.removed) delete next[key];
  for (const [key, changes] of Object.entries(patch.lists)) {
    const before = next[key] as { id: string }[];
    const byId = new Map(before.map(row => [row.id, row]));
    for (const row of changes.upsert) byId.set(row.id, row);
    next[key] = (changes.order ?? before.map(row => row.id)).map(id => byId.get(id)!);
  }
  return next as T;
}

// Native snapshots contain JSON values. Reuse unchanged branches so receiving
// a tab title does not invalidate every space, setting and saved-page component.
export function shareSnapshot<T>(previous: T | undefined, incoming: T): T;
export function shareSnapshot(previous: unknown, incoming: unknown): unknown {
  previous = toRaw(previous);
  if (previous === incoming) return previous;
  if (!previous || !incoming || typeof previous !== "object" || typeof incoming !== "object") return incoming;
  if (Array.isArray(previous) && Array.isArray(incoming)) {
    const byId = new Map(previous.flatMap(value => value && typeof value === "object" && "id" in value ? [[value.id, value]] : []));
    const next = incoming.map((value, index) => shareSnapshot(
      value && typeof value === "object" && "id" in value ? byId.get(value.id) : previous[index], value,
    ));
    return next.length === previous.length && next.every((value, index) => value === previous[index]) ? previous : next;
  }
  if (Array.isArray(previous) || Array.isArray(incoming)) return incoming;
  const old = previous as Record<string, unknown>;
  const next = Object.fromEntries(Object.entries(incoming).map(([key, value]) => [key, shareSnapshot(old[key], value)]));
  return Object.keys(old).length === Object.keys(next).length && Object.keys(next).every(key => Object.hasOwn(old, key) && old[key] === next[key]) ? previous : next;
}

export function applySnapshot<T extends object>(current: T, incoming: T): void {
  const next = shareSnapshot(current, incoming);
  // Keep the reactive root too: effects reading activeTab should not invalidate
  // consumers that only read spaces or settings.
  for (const key of Object.keys(current) as (keyof T)[]) if (!Object.hasOwn(next, key)) delete current[key];
  Object.assign(current, next);
}
