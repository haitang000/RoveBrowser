# Third-party notices

## Zigzag

Copyright 2026 Zigao Wang. Licensed under the Apache License, Version 2.0.

Source: https://github.com/ZigaoWang/zigzag/tree/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d

Rove translates selected design constants and UI behavior from `Zigzag/Design/Tokens.swift`, `Design/DESIGN.md`, `Design/SpaceColor.swift`, `Design/Controls.swift`, `Sidebar/SidebarView.swift`, and `Zigzag/Peek/PeekController.swift`, and studies the Sidebar, CommandBar, Window, Tabs and Desk files listed in `docs/SOURCE_ANALYSIS.md`. The SpaceSwipe math, gesture rules, list handoff, presentation-value interruption and background color animation are adapted into `src/lib/space-swipe.ts`, `SpaceDeck.vue`, `SpaceList.vue` and `App.vue`. The derived CSS, geometry and motion are modified for Vue, CSS pixels, Windows fonts, Web Animations, DOM wheel events and the Windows title bar. No Swift source is compiled or copied wholesale. The independent Rust browser runtime, IPC, SQLite persistence, WebView2 snapshot capture and Vue state bridge are new implementations.

Full upstream license: [Apache 2.0](licenses/zigzag-APACHE-2.0.txt). Original attribution: [NOTICE](licenses/zigzag-NOTICE.txt). Trademark terms: [TRADEMARKS](licenses/zigzag-TRADEMARKS.md). Zigzag's name, app icon, Z mark and wordmark are NOT included as Rove product branding. Rove is not endorsed by Zigao Wang.

## Additional translated Zigzag behavior

The saved-page lifecycle and selected algorithms in `native/src/bookmarks.rs` and `src/lib/bookmarks.ts` are additionally translated from Zigzag's `Tabs/TabManager.swift`, `App/Library.swift`, `Sidebar/SidebarMenus.swift` and `Tabs/Tab.swift`. They are modified for the existing SQLite session schema, stable bookmark IDs, Vue rendering and Windows WebView2 navigation and controller ownership. See `docs/PINNED_TABS_PORT.md` for the behavior mapping and adaptation limits. Copyright 2026 Zigao Wang; Apache-2.0.

## Direct implementation dependencies

Versions are fixed in package-lock.json and native/Cargo.lock. This list distinguishes direct dependencies from upstream features that Rove does not use.

| Dependency | License |
|---|---|
| Vue 3, Pinia, Vite, @vitejs/plugin-vue, vue-tsc, Vitest | MIT |
| TypeScript, Playwright | Apache-2.0 |
| Rust winit, wry | Apache-2.0 |
| webview2-com, include_dir | MIT |
| windows-rs, serde, serde_json, uuid, url, rusqlite, base64, image | MIT OR Apache-2.0 |
| directories | MIT OR Apache-2.0 |
| SQLite (bundled via libsqlite3-sys) | Public domain |
| Inter Variable via @fontsource-variable/inter | SIL Open Font License 1.1 |
| Microsoft Edge WebView2 Runtime | Microsoft proprietary runtime; installed separately, not bundled |

Inter: Copyright 2016 The Inter Project Authors (https://github.com/rsms/inter). Unmodified Latin and Latin Extended variable WOFF2 files are bundled locally. The font copyright and full OFL are preserved in `licenses/dependencies/npm/@fontsource-variable_inter-*/LICENSE`.

Transitive dependencies retain their own licenses. Locked Windows Rust dependencies and npm production dependencies are recorded in `licenses/DEPENDENCIES.json`; available license/notice texts are preserved under `licenses/dependencies`. Regenerate with `node scripts/collect-licenses.mjs`. No upstream Zigzag Reader/Readability, Sparkle, ad-block rules, font files, logo assets or extensions have been imported.
