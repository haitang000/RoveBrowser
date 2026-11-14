# Zigzag 0.9 → Rove 移植候选（2026-11-14）

本轮重新核对上游源码后给出的移植候选清单。此前的[源码研究](SOURCE_ANALYSIS.md)基于提交 `5338e99`（0.5.x 时代），后续各移植记录基于 `12183cb`（0.6.2）；本次把 `.reference/zigzag` 更新到上游 `main` 的 `626a464`（0.9.0 beta 8，2026-11-14），与 Rove 当前移植基线相差 **1,152 次提交**。因此 0.7–0.9 新增的功能也进入候选，而不只是旧版本遗留缺口。

文档只记录实际读过的 Swift 源码与已核对的 WebView2 绑定，不把更新日志的营销描述当作实现依据。行号均指本次检出。

## 结论速览

0.7–0.9 的新增功能里，与 Rove 现有架构（Windows、WebView2、Vue 受信 UI、独立网页 HWND、SQLite 整份会话 JSON）冲突最小、收益最大的一批集中在**页面层**：错误页、Cookie 横幅、标签休眠、历史页、站点权限、阅读器、打印/另存。**标签层**的归档、数字提示/切换器、侧栏尺寸/右侧改动小、风险低，适合成批做。广告拦截、4 分屏、嵌套标签、扩展、Profiles、Web Apps、Zigzag Mini 属于大工程，应排在第二批之后。翻译在 WebView2 上没有可用引擎，不建议移植。

## Rove 现状（0.1.0-beta6，用于对比）

已具备：工作区/空间与切换动画、每空间标签与固定标签、跨空间全局 Favorites（≤8）、两栏分屏（原生拖动缩放，比例不持久化）、标签悬停预览、Peek、Desk 全部标签页、命令栏（地址/搜索/站内搜索/切标签/历史/建议/内联补全/动态命令）、下载面板、页内查找（WebView2 原生 Find）、设置 7 页（含跨页搜索）、首启引导与浏览器数据导入、DPAPI 密码填充、默认浏览器、会话恢复、站点缩放/静音/播放指示、Clear 与撤销、玻璃/圆角/阴影/原生 Snap 标题栏。

明确缺失（与本文候选对应）：自定义错误页、Cookie 横幅、标签休眠、标签归档、嵌套标签、>2 栏分屏、右置侧栏、历史页、站点权限 UI、通知/页面对话框、阅读器、强制深色、广告拦截、扩展、Profiles、Web Apps、自动化/MCP、自动更新、站点数据管理、整页截图、打印/另存。

## 优先级总览

| # | 候选 | Zigzag 依据 | 价值 | 工作量 | 风险/依赖 |
|---|---|---|---|---|---|
| **P0** | | | | | |
| 1 | 自定义错误页（19 类）+ 代理提示 + 失败不入历史 | `Web/ErrorPage.swift`、`Web/ProxyCheck.swift`、`Web/NetworkChanges.swift` | 高 | M | 低 |
| 2 | Cookie 横幅自动拒绝（Autoconsent） | `Web/CookieBanners.swift` + `Resources/Autoconsent` | 高 | S–M | MPL-2.0，需页面内消息垫片 |
| 3 | 标签休眠（内存节省） | `Tabs/TabSleep.swift`、`Tabs/PageState.swift` | 高 | M | WebView2 `TrySuspend` 实测 |
| 4 | 打印 / 另存网页（含整页 PDF） | `Web/PDFViewer.swift`、`Window/BrowserActions` | 中高 | S | 低 |
| 5 | 历史页 + 40 分钟会话聚类 + 撤销清除 | `Web/HistoryPage.swift`、`Web/HistoryClusters.swift` | 高 | M | 需 `history` 表迁移 |
| 6 | 站点权限对话框 + 权限存储（最小闭环） | `Web/SitePermissions.swift` | 高 | M | 原生 `PermissionRequested` + deferral |
| **P1** | | | | | |
| 7 | 阅读器 Reader | `Web/Reader.swift` + Readability | 高 | M | Apache-2.0 |
| 8 | 深色页面（先滤镜，后 Dark Reader） | `Web/DarkPages.swift`、`DarkReaderFixes`、`DarkSeen` | 高 | M–L | 跨域 CSS/图片需宿主代理 |
| 9 | 标签归档（Tidy） | `Tabs/TabArchive.swift` | 中高 | M | 默认关闭 |
| 10 | 页面对话框 + HTTP 认证 | `Web/PageDialogs.swift` | 中 | S | 低 |
| 11 | 通知 + 弹窗拦截提示 | `Web/WebNotifications.swift`、`Web/PopupNotice.swift` | 中 | S–M | Windows Toast |
| 12 | 视频占满页面（页内全屏） | `Web/PageFullScreen.swift` | 中 | S | 页内脚本 + 布局 |
| 13 | Now Playing 侧栏卡片 | `Sidebar/NowPlaying.swift` | 中 | S | 复用现有音频状态 |
| 14 | 数字键提示 + Ctrl+Tab 切换器 + 空间切换器 | `Sidebar/NumberHints.swift`、`Switcher/*` | 中高 | S–M | 纯 UI |
| 15 | 侧栏尺寸 Small/Medium/Large + 右侧侧栏 | `Sidebar/SidebarView.swift`、`BrowserRootView` 镜像布局 | 中高 | S / M | 原生页面矩形需镜像 |
| 16 | 新标签排列选项 + 分屏比例持久化 + 分屏关闭按钮 | `Tabs/TabManager.swift`、`Tabs/TileLayout.swift` | 中 | S | 低 |
| 17 | 导入扩展：Arc 空间/固定页/打开标签 + 更多 Chromium 浏览器 | `Import/ArcImporter.swift`、`Import/ChromiumImporter.swift` | 高 | M | Windows 版 Arc 数据路径待核 |
| 18 | 整页截图 | `Web/PageScreenshot.swift` | 中 | M | CDP `captureBeyondViewport` 或滚动拼接 |
| **P2 / 大工程** | | | | | |
| 19 | 广告拦截后端 | `Web/AdFilters.swift`、`Web/ContentBlocker.swift`、`BlockedScriptStandIns` | 高 | L | 列表 GPL-3.0 只能运行时下载 |
| 20 | 4 栏分屏（TileLayout） | `Tabs/TileLayout.swift`、`PaneHeader.swift`、`PaneDrag.swift` | 高 | L | 4 个 WebView2 的内存/GPU 开销 |
| 21 | 嵌套标签（TabTree） | `Tabs/TabTree.swift` | 高 | M–L | 触及所有列表操作 |
| 22 | 扩展支持 | `Extensions/*` | 中 | L | 产品风险：WebView2 只有部分 chrome.* API |
| 23 | Profiles（按空间的数据隔离） | `Profiles/*`、`docs/plans/profiles.md` | 中高 | L | 架构级；跨 profile 需重建网页 |
| 24 | Web Apps（安装为应用） | `WebApps/*` | 中 | L | 需多窗口模式与快捷方式 |
| 25 | Zigzag Mini（Little Arc 小窗） | `Window/MiniWindowController.swift` | 中高 | L | 需多窗口 + 网页状态交接 |
| 26 | MCP / AI 自动化 | `MCP/*`、`docs/automation.md` | 中高 | M–L | 安全面、需本地 HTTP 服务 |
| 27 | 自动更新 | `App/Updater.swift`、`App/UpdateRelaunch.swift` | 中 | M | 发布流程/签名 |

筛选规则：能直接复用 Rove 现有受信 UI、原生页面矩形、`Command` 枚举与 SQLite 的功能优先；依赖多窗口、网页状态交接或大规模 API 桥接的放到 P2。

---

## P0 详细方案

### 1. 自定义错误页 + 代理提示

**现状**：导航失败时显示 WebView2 自带 Chromium 错误页（`IsBuiltInErrorPageEnabled` 默认开启），界面只有一条 `ErrorToast`；失败的导航仍会按普通加载路径处理，没有“失败不入历史”的分支。

**Zigzag 机制**（`Web/ErrorPage.swift`）：
- 19 种错误类型（`:13`，offline / notFound / refused / timedOut / secureFailed / certificate / clientCertificate / redirects / broken / badAddress / port / blocked / filtered / missingFile / unreadableFile / unavailable / extensionGone / stopped / other），各配文案、图标与按钮顺序；主按钮由回车触发。
- 证书错误提供“仍要访问”，用一次性 nonce 链接 + 24 小时有效期，避免页面伪造绕过；HSTS 站点不提供绕过（`:21-24`、`:60-79`）。
- 进程连续两次崩溃的页面用 `crashDomain = "ZigzagPageStopped"`（`:9`）区分“页面反复停止”。
- 弱表标记“下一次提交是错误页”，使错误页不被记成一次访问；离线错误在恢复联网后自动重试（`OfflineRetry`）。
- 空 5xx（代理返回的空响应）也当成 broken 页，而不是白页（`:84-89`）。
- `ProxyCheck.swift`：只认本机回环代理（127.0.0.1/localhost/::1），失败时提示“你的代理可能没连到”，而不是笼统报网络错误。

**Rove 落地**：
1. 在 `register_page_events` 补 `NavigationCompleted`（判断 `IsSuccess` 与 `WebErrorStatus`）与 `ProcessFailed`（计数后给 stopped 文案）；导航失败时不动网页，改为让前端展示内部错误页。
2. 复用“内部页面”模式：Rove 的设置页已经是受信 UI 中的一个内部页面（`rove://settings`）。新增 `rove://error` 页面，由 Vue 用现有 tokens 渲染，`PageArea` 隐藏该标签对应的网页 HWND；`Try Again` 通过现有 `navigate` 命令重试，`Go Back` 用现有 `back`。
3. `ICoreWebView2Settings::SetIsBuiltInErrorPageEnabled(false)` 关闭原生错误页；证书失败用 `ServerCertificateErrorDetected` 拿到错误并交给 Vue，选择“仍要访问”时设置 `COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW` 并重新导航（会话级；不做持久信任）。
4. 错误页状态放在标签上（`unloaded`/`error` 之外新增运行态字段，不落会话）；历史记录路径加“本次导航失败不记录”。
5. 代理提示读 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` 的 `ProxyEnable`/`ProxyServer`，仅回环代理时附加说明。

**验收**：本地 fixture 制造 DNS 失败、拒绝连接、超时、空 5xx、证书错误；断言内部错误页出现、主按钮可重试、历史表无该 URL、导航成功后退回正常网页；`cargo test` 覆盖错误分类映射。

**工作量**：M（主要是 Vue 错误页与状态接线）。风险低。

### 2. Cookie 横幅自动拒绝（Autoconsent）

**现状**：Rove 完全没有 Cookie 横幅处理。

**Zigzag 机制**（`Web/CookieBanners.swift` + `Resources/Autoconsent`，共约 600 KB）：
- DuckDuckGo Autoconsent 的 Playwright 版内容脚本（`:13-16`，`autoconsent.playwright.js` 157 KB + compact rules 445 KB，MPL-2.0 原样引入）。
- 每个 frame 在 document-start 注入；Zigzag 充当 background：回 `init`（配置 + 本 frame 适用的规则）、执行规则要求的 `eval`、收 `popupFound`/`optOutResult`/`autoconsentDone`。
- 默认 `reject`：能拒绝就拒绝，不能拒绝只在规则判定安全时隐藏，**从不接受**（`:4-7`、`:204-222` 关闭启发式与自动接受）。
- 站点级 `handled`/`left` 覆盖 + 全局 `reject`/`leave` 模式；与广告拦截互相独立。

**Rove 落地**：
- WebView2 的网页没有宿主 IPC，但 Autoconsent 只需要两个页面内全局函数：`window.autoconsentSendMessage` 与 `window.autoconsentReceiveMessage`。注入一段自包含垫片即可在本页内完成“background”角色：`init` 时用页面内规则索引（构建期从 compact rules 预生成 `host → rules` 映射）返回 `initResp`；`eval` 直接在本页 world 执行（限制 `EVAL_` 前缀与长度上限）；其余消息仅记录结果。
- 用 `AddScriptToExecuteOnDocumentCreatedAsync` 注入（所有 frame）；设置页增加“Cookie 横幅：自动拒绝 / 不处理”，站点级开关可在后续站点面板补；站点结果通过 `ExecuteScript` 上报或先只在设置页展示统计。
- 由于注入在页面主 world（WebView2 没有隔离 world），需要 IIFE 包裹 + 随机化消息名，避免与站点脚本冲突；这是与 Zigzag 的最大差异，需实测 OneTrust/Cookiebot 等站点。
- 资源体积约 600 KB 嵌入 `dist`，对 Rove（当前 Release 约 4.3 MiB）可接受。

**验收**：本地 fixture 模拟三种常见 CMP 弹窗 DOM；断言默认模式不出现“接受”路径、Cookie 写入符合规则；`leave` 模式不改 DOM；单元测试覆盖模式/站点覆盖判定。

**工作量**：S–M。风险：无隔离 world 带来的脚本冲突与规则索引初始化耗时（可延迟到 idle）。

### 3. 标签休眠（内存节省）

**现状**：后台页面一直存活（`host.rs` 中 `pages: HashMap<String, Page>`，隐藏但不销毁，`:139`、`:252`），只有固定项可以“未加载”；标签老化只是视觉淡出。

**Zigzag 机制**（`Tabs/TabSleep.swift`）：
- 纯判定函数 `sleepers(candidates, awake:memory)`（`:47-68`），按最久未看排序：默认 **1 小时**（`:41`），同时存活超过 **20** 个时 **15 分钟**（`:42-43`），内存吃紧时 **5 分钟**（`:44`）。
- 永不休眠：在屏、播放声音、通话/PiP、来自会发通知的站点、`pageState.hasTypedText`（`:52`）；Favorites 只在内存 critical 时休眠，`KeepAwake` 站点最后休一个（`:53-66`）。
- 内存压力来自 macOS 的 `DispatchSource.makeMemoryPressureSource`，2 分钟有效窗口 + 30 秒重扫节流；休眠动作走 `StagedQueue` 每 0.1 秒 2 个（`:211-262`），避免一次性拆页面卡主线程。休眠后悬停卡显示“已休眠以节省内存，释放约 180 MB”。
- 唤醒恢复由 `PageState` 补充（`Tabs/PageState.swift`）：记录滚动位置与用户输入字段（30 个字段 / 20 万字符，只记录 `isTrusted` 输入，排除密码/卡号/OTP 类字段），唤醒后重试 20×250ms 恢复滚动与输入。

**Rove 落地**：
- WebView2 自带 `ICoreWebView2_3::TrySuspend` / `Resume`（`CanTrySuspend` 先查询），对不可见页面最直接；`Page::set_visible(true)` 会恢复。
- 状态机：隐藏页面进入候选；`awake` 数量即 `pages` 中非休眠数；定时器（30–60 秒粒度）跑 `TabSleep` 同样的阈值；内存压力用 `CreateMemoryResourceNotification(LowMemoryResourceNotification)` + `WaitForSingleObject` 线程，或 `GlobalMemoryStatusEx` 轮询。
- 跳过条件：当前标签、分屏伙伴、`IsDocumentPlayingAudio` 为真、Peek/preview 临时页、正在输入（第一版可先不做输入检测，Phase 2 用 `PageState` 的 JS 预注入记录 `isTrusted` 输入并上报）。
- UI：侧栏图标加虚线环 + 悬停卡文案；设置页“标签”分组加开关；`TrySuspend` 失败或释放量太低时退回“销毁页面、点击时按 URL 重建”（会丢失页内状态，文案需区分“已卸载”与“已休眠”）。
- 若 `TrySuspend` 对复杂页面释放有限（渲染进程仍常驻），报告应诚实写“节省内存”，不要编造具体 MB；可选用 `GetProcessExtendedInfos`/`GetProcessMemoryInfo` 做估算。

**验收**：Rust 单测覆盖阈值/跳过条件（对齐 Zigzag 的参数）；原生冒烟用多标签 + 本地页面断言隐藏页面进入 suspend、切回后页面可见且 URL/title 保持；测量前后进程内存。

**工作量**：M。风险：`TrySuspend` 在旧 WebView2 Runtime 上不可用需降级；对 SPA 的滚动恢复不保证。

### 4. 打印 / 另存网页

**现状**：无打印、无另存为。

**Zigzag 机制**：网页走系统打印面板；另存页/PDF 时保留原页码（`Web/PDFViewer.swift` 的打印与另存路径，0.7+ 完善）。

**Rove 落地**（全部原生，无需网页 IPC）：
- `ICoreWebView2_16::ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_SYSTEM)` → Ctrl+P 与命令栏“打印…”；
- `PrintToPdfAsync(path)` / `PrintToPdfStream` → 保存 PDF；保存路径用 Windows 保存对话框（`native/src/passwords.rs` 已有 `GetOpenFileNameW` 模式，补 `GetSaveFileNameW`）；
- `ICoreWebView2_25::ShowSaveAsUI(COREWEBVIEW2_SAVE_AS_KIND_SINGLE_FILE)`（若 Runtime 支持）→ 另存网页；旧 Runtime 回退为“另存 PDF”。
- 快捷键 `Ctrl+P`、`Ctrl+S` 进设置页快捷键表。

**验收**：本地 fixture 打印为 PDF 后校验页数与文字；另存文件存在且可再次打开。

**工作量**：S。风险：`ShowSaveAsUI` 依赖较新 Runtime，需运行时探测接口。

### 5. 历史页 + 会话聚类 + 撤销清除

**现状**：历史只存在于命令栏搜索；`storage.rs` 的 `history(url TEXT PRIMARY KEY, title, visited)` 无 host / 访问次数，`visit()` 每次覆盖时间戳。

**Zigzag 机制**（`Web/HistoryPage.swift`、`Web/HistoryClusters.swift`）：
- 历史是 `zigzag://history` 内部页（`:4-18`），由应用自身提供、只接受受信消息；支持搜索、清除（最近一小时/今天/昨天/全部）并可撤销、分批加载、按行网站图标。
- 会话聚类：40 分钟无访问即断场（`HistoryClusters.swift:8`），同一场里连续访问同一站点折叠成一行；不跨午夜。

**Rove 落地**：
- SQLite 迁移：`ALTER TABLE history ADD COLUMN host TEXT / visits INTEGER`，或新建 `history_visits(url,title,host,visited)` 保留逐次访问（撤销清除需要保存被清的行，Zigzag 用归档标签 + 撤销栈；Rove 可在清除时先把行读出交给前端）。
- 新增 `rove://history` 内部页（复用设置页的内部页面机制与 tokens）；Rust 提供 `historyPage { query, before, limit }` 与 `historyClear { range }` 命令；聚类算法放 Rust（可单测）。
- 命令栏现有历史建议保持不动，历史页作为完整管理器。
- 清除历史时同步清空 `closed` 栈（现有行为保持一致）。

**验收**：Rust 单测覆盖 40 分钟断场、同站折叠、午夜边界、分页；界面测试覆盖搜索/清除/撤销/深色；原生冒烟确认访问记录与重启恢复。

**工作量**：M（表迁移 + 新内部页）。风险低。

### 6. 站点权限对话框 + 权限存储（最小闭环）

**现状**：`native/src` 里没有任何 `PermissionRequested` 处理；WebView2 默认策略下摄像头/定位/通知等请求的表现不可控，设置页也没有权限管理。

**Zigzag 机制**（`Web/SitePermissions.swift`）：
- `SiteOrigin` = scheme + host + port（省略默认端口，`blob:` 继承创建者），权限按 origin 存（`:9`、`:69-70`）。
- 关键设计：**“询问”是每次访问都真的问**——Allow/Block 才持久化；同一页面内的回答只对本次页面有效（`PagePermissionAnswers`，`:52`）。只有站点真的问过，站点面板才列出该权限（`noteAsked`）。
- 种类：摄像头/麦克风、位置、通知、弹窗、打开外部应用；提示是浮在页面上的面板，标题带来源 host（跨域 iframe 也标出来源）。

**Rove 落地**：
- 原生注册 `ICoreWebView2::add_PermissionRequested`（拿 `Uri`/`PermissionKind`/`GetDeferral`），不用 wry 的 `with_permission_handler`（它拿不到来源站点）；用 popup 层显示 Vue 询问卡片，答案经新 `Command::PermissionAnswer { id, kind, state, remember }` 回到宿主，再 `args.SetState` + 完成 deferral。
- 持久化：新表 `site_permission(origin TEXT, kind TEXT, state INTEGER, updated INTEGER, PRIMARY KEY(origin,kind))`；“总是允许/阻止”写入并用 `ICoreWebView2Profile4::SetPermissionState` 预置到 profile，减少重复询问；“这次允许”只完成当前 deferral。
- 第一版覆盖 camera/microphone/geolocation/notifications/clipboard-read；弹窗授权后续接 `NewWindowRequested`。
- 设置页隐私组增加“站点权限”列表（按 origin 展示已记住的项，可重置）；站点控制面板（地址栏按钮）作为后续增强。

**验收**：本地 fixture 分别请求摄像头/定位/通知；断言询问出现一次、允许后页面拿到权限、记住后不再问、私密语义（若做隐私窗口）不写盘；Rust 单测覆盖 origin 规范化（端口/IPv6/blob）。

**工作量**：M（提示 + 存储 + 设置列表）；含完整站点面板与证书链则 L（证书链在 P1 之后单独做）。

---

## P1 方案要点

7. **阅读器 Reader**（M）：引入 Mozilla Readability（Apache-2.0，`Readability.js` 93 KB + `Readability-readerable.js` 4 KB），按需注入页面，`isProbablyReaderable({minScore:10, minContentLength:100})`（`Reader.swift:30`）判断；提取后给页面套用受控阅读模板（正文/标题/作者/时间、Serif/Sans、窄/中/宽栏、Light/Dark/Auto/Sepia 主题，`Reader.swift:252-271`），Esc 返回原页面并恢复滚动。也可后续改成内部 `rove://reader` 页以彻底禁止脚本；第一版页内替换即可，注意链接路由与 `document.title`。验收：新闻页/维基页进入与退出、图片加载、页内查找可用。

8. **深色页面**（M–L，建议分两步）：Phase 1 用 CDP `Emulation.setEmulatedMedia` 强制 `prefers-color-scheme: dark` + 注入低侵入滤镜 CSS（Zigzag 0.8 之前的方法），按站点开关并持久化；Phase 2 引入 Dark Reader（MIT，`darkreader.js` 359 KB + fixes 871 KB）做按站点重绘。后者需要取跨域样式表与图片分析，Zigzag 用原生代理（`DarkPages.swift:4-16`、`DarkReaderFixes`）；Rove 网页无 IPC，可由宿主在 `WebResourceRequested` 里代取 CSS 再注入，或先接受跨域站点降级。`DarkSeen` 的“站点自带深色则从首帧就深色”值得移植为早期注入脚本。

9. **标签归档 Tidy**（M）：`Archive.json` 等价物放 SQLite；阈值 12 小时/1 天/1 周/永不（默认永不，`TabArchive.swift:18-44`），保留 30 天 / 500 条；≥20 个标签时一次性提示（`:47-55`）；命令栏可搜归档项。风险：静默消失最招用户恨——默认关闭、给可见入口。

10. **页面对话框 + HTTP 认证**（S）：`ScriptDialogOpening`（alert/confirm/prompt，带 deferral）→ popup 层 Vue 卡片，标题“<站点> 说”；`BasicAuthenticationRequested` → 用户名/密码卡片；文件选择保留 WebView2 默认。对照 `PageDialogs.swift:44-55`、`:218`。

11. **通知 + 弹窗拦截提示**（S–M）：`NotificationReceived`（新版 Runtime）或不注入 shim 的降级路径；Windows Toast 用 `windows` crate 的 Toast API 或应用内通知；权限走 P0-6。弹窗拦截：`NewWindowRequested` 在 `IsUserInitiated == false` 时不再一律开标签，改为拦截并提示（每窗口/站点 10 秒节流），对照 `PopupNotice.swift`。Rove 现有 popup 处理在 `host.rs:2954-2988`。

12. **视频占满页面**（S）：`PageFullScreen.swift:9-11` 的做法是在页内接管 `requestFullscreen`/`webkitEnterFullscreen`，把元素固定到视口并伪造 `fullscreenElement`/`fullscreenchange`；WebView2 中页内全屏是整窗全屏，若要“只占页面区域”，在页面内用 CSS 固定即可（网页 HWND 本来就是页面矩形），无需原生改动；加设置项与站点开关。

13. **Now Playing 卡片**（S）：`Sidebar/NowPlaying.swift:3-13` 是侧栏底部的一行（站点、播放/暂停、静音、关闭，固定显示），按 (标签, 页面) 记住隐藏；Rove 已有 `playing` 集合与 `is_playing_audio`，补注入脚本控制媒体元素播放/暂停即可。

14. **数字键提示 + 切换器**（S–M）：`ModifierHold` 0.5 秒（`NumberHints.swift:37-74`），只按修饰键且无其他输入时显示；标签 ⌘1–⌘8、⌘9 最后，空间 ⌃1–⌃9；`Ctrl+Tab` 网格切换器（`Switcher/TabSwitcher.swift`）与 ⌥⌘S 空间切换器（`SpaceSwitcher.swift`）。Rove 已有 Ctrl+Tab 循环与 Ctrl+1–9，补“按住显示数字 + 切换器浮层”，键盘事件经原生 accelerator 转发到受信 UI。

15. **侧栏尺寸 + 右侧侧栏**（S / M）：0.9 的 Sidebar size 是 0.9 / 1 / 1.15 三档缩放（`#183`，行高 30、图标 16、空间名 26/36 等基数）；右侧侧栏是把所有左侧布局镜像（`BrowserRootView` 的 `mirrored`），Rove 需要同步镜像原生页面矩形、标题栏按钮顺序与浮层锚点。

16. **新标签排列 + 分屏持久化**（S）：`TabManager.newTabPlace` 支持“当前标签旁 / 顶部 / 底部 / 同站标签旁”；分屏比例写入会话（Rove 目前只存组合，`splitWith` 已持久化，比例没存）；分屏每个面板一个关闭按钮（`PaneHeader` 的 Close Tab）。

17. **导入扩展**（M）：Arc 导入空间名/颜色、固定页、Favorites、每个空间的打开标签（未加载，`ArcImporter.swift`）；Chromium 系浏览器列表扩到 Helium/Comet/Yandex/Whale/Chromium/Thorium 等，Windows 上数据布局与 Chrome 相同；Chrome 127+ 的密码 app-bound 加密（`v20`）无解时只导入书签/历史并提示 CSV。这是拉新最强的功能。

18. **整页截图**（M）：`PageScreenshot.swift` 先滚动一遍触发懒加载（上限 40 屏 / 32,000 px / 1.5 亿像素，`:19-22`），再整页合成；WebView2 用 CDP `Page.captureScreenshot {captureBeyondViewport:true, clip}`，超 16,384 px 分块拼接，失败回退滚动拼接。

## P2 大工程要点

- **广告拦截（L）**：WebView2 没有声明式 content blocker。Rust 侧用 `adblock-rust`（MPL-2.0）匹配 `WebResourceRequested` 取消请求，再注入 cosmetic CSS/scriptlet；uBO 列表 GPL-3.0 **只能在运行时下载**（不随包分发），EasyList 选 CC BY-SA 3.0，`zigzag-fixes` 模式可借鉴；先做域名级黑白名单骨架，再上完整规则。性能是主要风险（每个子资源都过宿主）。
- **4 栏分屏（L）**：`TileLayout.swift` 是纯几何（2×2 网格、4 bit region 掩码、`frames(in:gap:)`、divider 命中、`placing`/`removing` 重排，`:6-190`），可原样移植为 TS；Rove 需把两栏布局器升级为网格、每个面板独立标题条（`PaneHeader`）、拖放预览、会话持久化 regions/ratio。Zigzag 自己为 #160 打了约 10 个修复，排期要留出来。
- **嵌套标签（M–L）**：`TabTree.swift` 全是纯函数（levels/repaired/subtreeEnd/descendants/closing/insert 规则，`:7-120`），移植为 TS 模块即可；但会触及新建/关闭/拖动/固定/归档/分屏的每条路径，应先于归档与批量操作做。
- **扩展支持（L，产品风险）**：好消息是 wry 0.57.1 已暴露 `with_browser_extensions_enabled` / `with_extensions_path`，底层调用 `ICoreWebView2Profile7::AddBrowserExtension`，技术可行；但 WebView2 只支持解包目录、`chrome.*` 子集（无工具栏 popup、无 side panel、无 nativeMessaging），也没有商店安装流程。半可用扩展比“暂时没有”更伤信任，建议保持不做。
- **Profiles（L）**：WebView2 有 `ProfileName` 可按空间隔离 Cookie/存储/扩展，但 Rove 是“一个数据目录一个进程”，需要 `profiles` 表、页面创建时带 profile、历史/收藏按 profile 归属、跨 profile 移动标签 = 重建网页。
- **Web Apps / Zigzag Mini（L）**：都需要多窗口架构（Rove 当前单窗口）。Windows 上 Web App = `rove.exe --app <id>` + 生成 `.ico` 快捷方式 + AppUserModelID；Mini = 第二窗口 + 标签状态交接（WebView2 无法把网页在窗口间搬移而不重建）。
- **MCP（M–L）**：本地 HTTP 服务（127.0.0.1 + Bearer token）暴露 list/open/navigate/read/click/type 等工具，页面操作走 `ExecuteScript`；可以作为下一步差异化能力，也能当测试夹具。
- **自动更新（M）**：Windows 侧需要下载器 + 安装器静默升级（现有 Inno Setup + 便携 ZIP），并把更新前后的会话保存成可恢复文件。

## 不建议移植

| Zigzag 功能 | 原因 |
|---|---|
| 页面翻译（Apple Translation） | WebView2 无翻译引擎；本地 NMT 模型 100 MB+/语言对，云 API 破坏 Rove 的隐私边界。DOM 分段/批处理逻辑可留作以后接自建引擎。 |
| AppleScript / Shortcuts 自动化 | macOS 专属；要脚本化就走 MCP 或本地 HTTP。 |
| Keychain / macOS 27 Autofill 面板 / Touch ID | Windows 无对应面板；Rove 的 DPAPI + 自有填充 UI 已是等价路径。密码生成器/健康检查/保存提示可单独移植（逻辑 S）。 |
| Passkeys 特判 | Windows/Chromium 原生支持 WebAuthn，不需要移植 Zigzag 的 entitlement 变通。 |
| Sparkle 更新、MoveToApplications、MetricKit、菜单栏/Dock/⌘Tab 集成 | 平台专属；Windows 有各自的等价实现。 |
| SafePictures 的 XPC 解码器 | Rust `image` 解码器内存安全；保留字节/像素上限与超时策略即可。 |
| HSTS 自有列表、Alternative Services 清理、HTTPS-First | Chromium 自带 HSTS/升级且无对应清除 API；移植只会重复造轮子。 |
| 自绘 PDF 查看器 / PDFKit | WebView2 自带 PDFium 查看器；除非要做“阅读位置记忆 + 目录侧栏”，优先级低。 |
| 屏幕共享选择器、WebKit 私有 API、SF Symbols | 平台专属或无公开 API。 |
| 扩展支持（当前阶段） | 技术可行但体验残缺（见 P2-22），产品风险高于收益。 |

## 0.7–0.9 新功能全景

| 版本 | 新功能 | 可移植性 | Rove 状态 |
|---|---|---|---|
| 0.7.0 | Clear 保留近期标签；新标签位置；按住 ⌘ 显示数字；后台标签能耗休眠；Zigzag Mini；⌥⌘S 空间切换器；扩展代理；开发者工具快捷键；文件夹增强 | 高（数字/切换器/位置/Clear）；Mini 低 | Clear/标签位置部分已有；数字/切换器/Mini/休眠无 |
| 0.7.1 | Mini 重建；Ctrl+Tab 网格切换器 | 高 | Ctrl+Tab 循环已有，网格无 |
| 0.7.2 | Mouse side buttons；设置可关闭捏合；快捷键移除 | 高 | 无鼠标侧键、无捏合开关 |
| 0.7.3 | 扩展填充；拖动空间图标排序；密码扩展支持 | 中（空间排序 S） | 无扩展 |
| 0.7.4 | Mini 边框模式 | 低 | — |
| 0.7.5 | 设置分节；PiP 修复；侧滑改进 | 中 | 设置已分页 |
| 0.7.6 | 扩展导入；AppleScript | 中 / 否 | 仅书签历史导入 |
| 0.7.7 | 标签休眠时间(#32)；后台标签恢复；命令栏不阻塞历史 | 高 | 无休眠；命令栏已有内联补全 |
| 0.7.8 | What's New；面板/菜单动效；侧滑手势设置；长站点名 | 高（What's New S） | 动效部分已有 |
| 0.7.9 | 更多浏览器导入；地址栏上置；扩展固定；⌃ 数字；⌥⌘ 拖动窗口；错误页 | 高（错误页/导入/数字） | 无 |
| 0.8.0 | Profiles；Dark Pages；嵌套标签；Mini Player；地址栏随页面着色；Reader；分屏单页关闭；Keep on Top | 高（Reader/深色/嵌套）→大工程（Profiles/Mini） | 无 |
| 0.8.1 | Keep Awake 站点；空会话提示；全页查找计数改进 | 高 | 无 |
| 0.8.2–3 | Profiles 修复；切换器布局修复 | — | — |
| 0.8.4 | 密码 App 填充；YouTube 广告；多 profile 导入；睡眠环；设置搜索 | 高（睡眠环/导入/设置搜索） | 设置搜索已有；其余无 |
| 0.8.5 | MCP；设置分组；站点图标自定义；Dark Reader 引擎；Mini Player 缩放；收藏夹图标镂空 | 中（MCP/深色/图标） | 无 |
| 0.8.6 | 设置分节；⌘Z 先给页面；窗口尺寸修复 | 高（⌘Z 冲突是已有问题可查） | 无 |
| 0.9.0 | 4 栏分屏；Web Apps；右侧侧栏；窗口间移动标签；Mini Player 全屏角落；侧栏尺寸；地址拖放；Space 默认色；Chrome 扩展导入；Site Controls 重建（证书链/权限/扩展）；Dark Reader 站点级；广告 uBO 列表；Cookie 横幅；标签翻译；睡眠标签；Now Playing 隐藏；图标 emoji 搜索 | 高（右侧/尺寸/Now Playing/Cookie）；中（广告/证书/翻译）；大工程（4 栏/Web Apps/窗口移动） | 空间图标/emoji 已实现（`d6cfdc7`）；其余无 |

## 建议的下一批实施顺序

按“互不耦合、可独立验收、每个都能单独进 VALIDATION”的原则分批：

1. **批次 A（页面体验，约 1 周）**：自定义错误页（P0-1）→ 打印/另存（P0-4）→ Cookie 横幅（P0-2）→ 弹窗拦截提示（P1-11 的前半）。四项都不改存储模型，只加命令/事件/前端页面，适合作为一批提交。
2. **批次 B（标签与资源）**：标签休眠（P0-3）→ 标签归档（P1-9）→ 数字提示/切换器（P1-14）→ Now Playing（P1-13）。休眠涉及 `host.rs` 页面生命周期，先做；其余都是前端/存储增量。
3. **批次 C（数据与信任）**：历史页 + 聚类（P0-5）→ 站点权限最小闭环（P0-6）→ 页面对话框（P1-10）→ 侧栏尺寸/右侧（P1-15）。历史页会改 `history` 表，之后做权限与站点数据都会复用 origin 索引。
4. **批次 D（内容能力）**：阅读器（P1-7）→ 深色页面（P1-8）→ 整页截图（P1-18）→ 视频占满页面（P1-12）。
5. **再往后**：导入扩展（P1-17，拉新）与 MCP（P2-26，差异化）可以独立插队；广告拦截（P2-19）、4 分屏（P2-20）、嵌套标签（P2-21）各自单独开分支，不要和别的批次混。

每批的建议做法（与既有文档一致）：

- 先写纯逻辑的 Vitest 单测（阈值、聚类、origin 规范化、规则判定），再接 UI；原生部分补 `cargo test` 与 `scripts/native-*.mjs` 冒烟脚本。
- 需要新 WebView2 接口的（`TrySuspend`、`ShowSaveAsUI`、`PermissionRequested`、`NotificationReceived`、`ShowPrintUI`）在宿主里做运行时 `cast` 探测，接口缺失时功能降级而不是崩溃；Rove 已有 `web_dispose`/接口 cast 的先例（`web_settings.rs`、`site.rs`）。
- 新持久化数据走新表或 `State` 的 `#[serde(default)]` 字段，并在 `repair()` 里做旧数据修正；不要破坏现有 `session` JSON 的兼容。
- 三方资源（Readability/Autoconsent/Dark Reader/adblock 引擎）必须按许可证要求放 `THIRD_PARTY_NOTICES.md` 与 `licenses/`，并记录版本与原始地址；GPL-3.0 的 uBO 列表不得随包分发。
- 每批完成后在 `docs/VALIDATION.md` 记录实际跑过的测试与局限，不把“能编译”当验证。

## 许可证提示

| 组件 | 许可证 | 使用方式 |
|---|---|---|
| Zigzag Swift 源码 | Apache-2.0 | 可转写；保留版权/NOTICE，新增文件标注来源 |
| Mozilla Readability | Apache-2.0 | 可随包分发，保留许可证 |
| Dark Reader | MIT | 可随包分发，保留版权与许可证；Zigzag 的 proxy 拆分方式可参考 |
| Ghostery adblocker 引擎 | MPL-2.0 | 以独立未修改文件分发时可与 Apache-2.0 项目共存，附许可证 |
| DuckDuckGo Autoconsent | MPL-2.0 | 同上；规则文件随包 |
| uBlock Origin / EasyList 列表 | GPL-3.0 / CC BY-SA 3.0 | **只运行时下载，不随包分发**（Zigzag 的做法，照做） |
| Unicode 表情名称数据 | Unicode License v3 | 可随包，保留版权 |
| adblock-rust（若采用） | MPL-2.0 | 独立依赖，更新 THIRD_PARTY_NOTICES |

Mac 端专属素材（截图、Logo、字体、SF Symbols）继续不复制；Rove 的图标体系保持自绘。
