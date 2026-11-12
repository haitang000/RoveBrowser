// Existing browser feature checks use an established profile. First-run
// behavior is covered separately by native-onboarding-smoke.mjs.
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
export function createLegacyNativeProfile(profile) {
  const db = new DatabaseSync(join(profile, "era.sqlite"));
  try {
    db.exec("CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL)");
    const state = {
      spaces: [["personal", "Personal", "sky"], ["work", "Work", "sage"], ["studio", "Studio", "lavender"]].map(([id, name, color]) => ({ id, name, color })),
      activeSpace: "personal", activeTab: "start", theme: "light", sidebarCollapsed: false, closed: [],
      favorites: [["GitHub", "https://github.com"], ["Figma", "https://figma.com"], ["Notion", "https://notion.so"], ["Claude", "https://claude.ai"], ["Gmail", "https://mail.google.com"], ["YouTube", "https://youtube.com"], ["Spotify", "https://open.spotify.com"], ["Linear", "https://linear.app"]].map(([title, url]) => ({ title, url })),
      tabs: [["github", "GitHub", "https://github.com", true], ["wiki", "Wikipedia", "https://en.wikipedia.org", true], ["start", "New Tab", "era://newtab", false]].map(([id, title, url, pinned]) => ({ id, title, url, pinned, spaceId: "personal" })),
    };
    db.prepare("INSERT INTO session(id,json) VALUES(1,?) ON CONFLICT(id) DO NOTHING").run(JSON.stringify(state));
  } finally { db.close(); }
}
