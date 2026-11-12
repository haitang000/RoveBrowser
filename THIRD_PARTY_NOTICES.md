# Third-party notices

## Zigzag

Copyright 2026 Zigao Wang. Licensed under the Apache License, Version 2.0.

Source: https://github.com/ZigaoWang/zigzag/tree/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d

Rove translates selected design constants and UI behavior from `Zigzag/Design/Tokens.swift`, `Design/DESIGN.md`, `Design/SpaceColor.swift`, `Design/Controls.swift`, `Sidebar/SidebarView.swift`, and `Zigzag/Peek/PeekController.swift`, and studies the Sidebar, CommandBar, Window, Tabs and Desk files listed in `docs/SOURCE_ANALYSIS.md`. The SpaceSwipe math, gesture rules, list handoff, presentation-value interruption and background color animation are adapted into `src/lib/space-swipe.ts`, `SpaceDeck.vue`, `SpaceList.vue` and `App.vue`. The derived CSS, geometry and motion are modified for Vue, CSS pixels, Windows fonts, Web Animations, DOM wheel events and the Windows title bar. No Swift source is compiled or copied wholesale. The independent Rust browser runtime, IPC, SQLite persistence, WebView2 snapshot capture and Vue state bridge are new implementations.

Full upstream license: [Apache 2.0](licenses/zigzag-APACHE-2.0.txt). Original attribution: [NOTICE](licenses/zigzag-NOTICE.txt). Trademark terms: [TRADEMARKS](licenses/zigzag-TRADEMARKS.md). Zigzag's name, app icon, Z mark and wordmark are NOT included as Rove product branding. Rove is not endorsed by Zigao Wang.

## Additional translated Zigzag behavior

The onboarding flow, centered step shell, welcome presentation, progress indicators, site selection and default-browser question in `src/components/browser/OnboardingPage.vue`, `src/styles/onboarding.css`, `src/lib/onboarding.ts` and `native/src/onboarding.rs` are adapted from `Zigzag/Import/Onboarding.swift`, `Import/WelcomeWindow.swift`, `Design/Brand.swift` and `Design/Tokens.swift` at commit `78edb3201b8ec1383bedf4f2eb3b3832af6c15f4`. Browser discovery and import in `DataImportPicker.vue`, `native/src/import.rs` and `native/src/import_store.rs` also adapt the upstream `ImportCoordinator.swift`, `ChromiumImporter.swift`, `FirefoxImporter.swift`, `CSVImporter.swift` and `ImportBatch.swift` to Windows, Rust and SQLite. Copyright 2026 Zigao Wang; Apache-2.0. Rove uses its own wordmark and adds typed IPC, SQLite snapshots and transactions, DPAPI credential storage, explicit account selection, progress and existing-profile compatibility. WebKit content blocking is not ported. See `docs/ONBOARDING_PORT.md`.

The saved-page lifecycle and selected algorithms in `native/src/bookmarks.rs` and `src/lib/bookmarks.ts` are additionally translated from Zigzag's `Tabs/TabManager.swift`, `App/Library.swift`, `Sidebar/SidebarMenus.swift` and `Tabs/Tab.swift`. They are modified for the existing SQLite session schema, stable bookmark IDs, Vue rendering and Windows WebView2 navigation and controller ownership. See `docs/PINNED_TABS_PORT.md` for the behavior mapping and adaptation limits. Copyright 2026 Zigao Wang; Apache-2.0.

The hover-preview delay, dismissal rules, grouped captions and snapshot-refresh behavior in `src/components/browser/TabPreview.vue` and `src/lib/tab-preview.ts` are adapted from Zigzag's `Sidebar/TabPreview.swift` and `Design/Tokens.swift`. Copyright 2026 Zigao Wang; Apache-2.0. The native cutout, rendering order, bounded in-memory thumbnail refresh and close acknowledgment are new Windows/WebView2 adaptations; no Swift implementation is compiled.

The find bar's capsule layout, search-as-you-type behavior, counter wording, step/wrap rules, Esc and Ctrl+F toggling and focus handling in `src/components/browser/FindBar.vue`, `src/lib/find.ts` and `native/src/find.rs` are adapted from Zigzag's `Window/FindBar.swift`. Copyright 2026 Zigao Wang; Apache-2.0. The implementation uses WebView2's `ICoreWebView2Find` for matching, highlighting and counts instead of Zigzag's injected counting script and private WebKit `_hideFindUI`; no Swift implementation is compiled. See `docs/VALIDATION.md` for the platform differences.

The settings page, searchable form grouping, responsive navigation, internal settings addresses, search engine definitions and shortcut customization are translated from `Settings/SettingsView.swift`, `Settings/SettingsPage.swift`, `Settings/Shortcuts.swift`, `App/Library.swift` and `Design/Tokens.swift` at upstream commit `12183cbaf5acf68ac4a23a1d5003b1b10d587e03`. Copyright 2026 Zigao Wang; Apache-2.0. The implementation uses Vue controls, Rust validation and SQLite session storage, with new Windows/WebView2 adapters for preferences, download destinations, data clearing and exit confirmation. See `docs/SETTINGS_PORT.md` for the supported behavior and platform differences.

The split resize input, minimum pane sizes, shared split boundary and immediate page layout in `native/src/split.rs` and `src/components/browser/PageArea.vue` are adapted from `Zigzag/Window/BrowserRootView.swift`, `Zigzag/Window/PageCardView.swift` and `Zigzag/Window/BrowserWindowController.swift` at commit `0f99cac833a9f23e379f617ae99800b5d6e2f19c`. Copyright 2026 Zigao Wang; Apache-2.0. The Win32 input window, mouse capture, WebView2 bounds updates and temporary removal of GDI clipping are Windows adaptations. See `docs/SPLIT_RESIZE_PORT.md` for the mapping and validation limits.

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
| Microsoft Edge WebView2 Runtime | Microsoft proprietary runtime; installed separately; the installer includes Microsoft's signed Evergreen bootstrapper |

Inter: Copyright 2016 The Inter Project Authors (https://github.com/rsms/inter). Unmodified Latin and Latin Extended variable WOFF2 files are bundled locally. The font copyright and full OFL are preserved in `licenses/dependencies/npm/@fontsource-variable_inter-*/LICENSE`.

Transitive dependencies retain their own licenses. Locked Windows Rust dependencies and npm production dependencies are recorded in `licenses/DEPENDENCIES.json`; available license/notice texts are preserved under `licenses/dependencies`. Regenerate with `node scripts/collect-licenses.mjs`. No upstream Zigzag Reader/Readability, Sparkle, ad-block rules, font files, logo assets or extensions have been imported.

## Windows installer

The Windows installer is generated with [Inno Setup](https://jrsoftware.org/isinfo.php). The Simplified Chinese messages in `installer/ChineseSimplified.isl` use the user-contributed translation from [Inno Setup 6.7.3](https://github.com/jrsoftware/issrc/blob/is-6_7_3/Files/Languages/Unofficial/ChineseSimplified.isl), maintained by Zhenghan Yang; only trailing whitespace is normalized, and the upstream attribution is preserved. The compiler is a build tool and is not included in the application payload.

Microsoft's signed WebView2 Evergreen bootstrapper is included in the installer and runs only when the runtime is missing. The runtime is downloaded from Microsoft and remains subject to [Microsoft's WebView2 Runtime terms](https://developer.microsoft.com/microsoft-edge/webview2/).
