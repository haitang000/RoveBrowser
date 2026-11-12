// Existing browser feature checks use an established profile. First-run
// behavior is covered separately by native-onboarding-smoke.mjs.
import { DatabaseSync } from "node:sqlite";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
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
// File-based CLI for PowerShell 5, whose native argument handling strips quotes
// from inline JavaScript. Also checks the installed application's saved URL.
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [profile, expectedUrl] = process.argv.slice(2);
  if (!profile) throw Error("A fixture profile directory is required");
  if (!expectedUrl) createLegacyNativeProfile(profile);
  else {
    const db = new DatabaseSync(join(profile, "era.sqlite"), { readOnly: true });
    try {
      const state = JSON.parse(db.prepare("SELECT json FROM session").get().json);
      if (!state.tabs.some(tab => tab.url === expectedUrl)) throw Error("Installed application did not save the external URL");
    } finally { db.close(); }
  }
}
