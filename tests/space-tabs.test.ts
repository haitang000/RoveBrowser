import { expect, it } from "vitest";
import { initialState, reduce } from "../src/lib/preview";

it("switching spaces restores the tab each space was showing", () => {
  let s = reduce(initialState(), { type: "new", url: "https://example.com/docs" });
  const personal = s.activeTab;
  s = reduce(s, { type: "space", id: "work" });
  const work = s.activeTab;
  s = reduce(s, { type: "new", url: "https://example.com/work-notes" });
  const notes = s.activeTab;
  // Leaving a space remembers the tab it was showing, not its first tab.
  s = reduce(s, { type: "space", id: "personal" });
  expect(s.activeTab).toBe(personal);
  s = reduce(s, { type: "space", id: "work" });
  expect(s.activeTab).toBe(notes);
  // A remembered tab that moved to another space falls back to the most
  // recently shown page instead of the first tab.
  s = reduce(s, { type: "space", id: "personal" });
  s = reduce(s, { type: "move", id: notes, spaceId: "studio" });
  s = reduce(s, { type: "space", id: "work" });
  expect(s.activeTab).toBe(work);
});

it("spaces without a remembered tab fall back to their most recent page", () => {
  // Sessions written before spaces remembered a tab still restore the page the
  // user looked at last rather than the first one.
  const s = initialState();
  s.tabs.push(
    { id: "studio-old", spaceId: "studio", title: "Old", url: "https://example.com/old", pinned: false, lastActivated: 10 },
    { id: "studio-recent", spaceId: "studio", title: "Recent", url: "https://example.com/recent", pinned: false, lastActivated: 20 },
  );
  expect(reduce(s, { type: "space", id: "studio" }).activeTab).toBe("studio-recent");
});

it("a shared favorite is restored into the space that remembered it", () => {
  let s = initialState();
  const favorite = s.favorites[0]!;
  s = reduce(s, { type: "bookmarkOpen", id: favorite.id });
  const favoriteTab = s.activeTab;
  s = reduce(s, { type: "space", id: "work" });
  s = reduce(s, { type: "bookmarkOpen", id: favorite.id });
  expect(s.activeTab).toBe(favoriteTab);
  expect(s.activeSpace).toBe("work");
  // The favorite's live tab belongs to Work now, but Personal still restores it.
  s = reduce(s, { type: "space", id: "personal" });
  expect(s.activeTab).toBe(favoriteTab);
  expect(s.activeSpace).toBe("personal");
});
