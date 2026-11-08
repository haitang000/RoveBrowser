# Zigzag → Rove 移植研究

研究基线：`https://github.com/ZigaoWang/zigzag`，提交 `5338e99f58aa9a75a0696d5d96ec74353c3b3c7d`。原始检出保留在 `.reference/zigzag`（不进入交付源码）。本文记录实际阅读的源码，不以 README 截图代替源码分析。

| 实际源码 | 阅读与迁移结论 | Rove 对应 |
|---|---|---|
| `Zigzag/Design/Tokens.swift` | 布局、Sidebar 专用尺寸优先于一般 Metric；透明度、圆角、动画 | `src/styles/tokens.css`、`theme.css`、`animations.css` |
| `Zigzag/Design/DESIGN.md` | frame → surface → selected card → floating 四层；空间着色、轻字体、无额外强调色 | 全局样式与组件 |
| `Zigzag/Design/SpaceColor.swift` | OKLCH 浅色 L=.915/C=.045，深色 L=.27/C=.04；sky 色度乘 .78 | extract-colors.mjs → palette.css，复用实际 OKLab/gamut 公式 |
| `Zigzag/Sidebar/SidebarRows.swift` | icon/text 双列、selected/hover/pressed 与 close 附件 | TabItem.vue |
| `Zigzag/Sidebar/SidebarListView.swift` | pinned/list 分段、拖动排序与跨空间移动、分屏意图 | Sidebar.vue、PinnedTabs.vue |
| `Zigzag/Sidebar/SidebarView.swift` | 顶部导航、地址、favorites、独立空间列表、底部数字键；SpaceSwipe 规则、相邻真实列表、方向切换与位置交接 | Sidebar.vue、WorkspaceSwitcher.vue、SpaceDeck.vue、SpaceList.vue、space-swipe.ts |
| `Zigzag/Design/Controls.swift` | `ColorView.setColor` 与 `CALayer.set(backgroundColor:)`：从当前 presentation 颜色开始、200ms 普通 ease 插值 | App.vue 的独立背景色 WAAPI，连续切换不中断列表/网页 |
| `Zigzag/Sidebar/FavoritesView.swift` | 四列 36 高 tiles，18 图标、6 间距 | Favorites.vue |
| `Zigzag/CommandBar/CommandBarController.swift` | 640 宽、56 高输入、40 高结果、同行标题/细节、上下键与焦点；原版 scrim 不调暗背景 | CommandBar.vue；当前适配为网页区域内浮窗，12% 遮罩及内存背景快照 |
| `Zigzag/CommandBar/CommandBarModel.swift` | open/search/switchTo/history 类型及上下文命令 | 命令结果选择；未迁移站内搜索、远程建议与完整命令集合 |
| `Zigzag/Tabs/Tab.swift` | 独立 WKWebView 生命周期、title/url/loading、页面与 host 分离 | native/src/host.rs、model.rs；改用 WebView2 |
| `Zigzag/Window/BrowserWindow.swift` | 自定义无标题窗口、拖动、最小尺寸 | winit + WindowControls 的 WebView2 原生拖动区域、caption.rs 原生缩放边框 |
| `Zigzag/Window/BrowserRootView.swift` | sidebar 默认240，网页 inset8，分屏间距8，页面/侧栏互不覆盖 | PageArea.vue + DOM bounds → Rust |
| `Zigzag/Window/PageCardView.swift` | 白色/深色页面表面、微弱接触阴影、页面圆角 | page-area CSS + native/src/clip.rs 原生 HWND 圆角区域 |
| `Zigzag/Desk/DeskController.swift` | 按空间分组、过滤、卡片与caption、拖动移动 | Desk.vue + capture.rs/thumbnails.rs；使用实际 WebView2 快照，分屏卡片并排显示两张 |
| `Zigzag/Peek/PeekController.swift` | 独立临时 tab，40 header，8% inset、Open as Tab | Peek.vue、peek-layout.ts、host.rs、capture.rs；同控制器晋升、临时生命周期、真实网页快照背景 |

## Space 触控板与背景动画移植

本次沿用上面的固定提交，实际移植 `SidebarView.swift` 的 `SpaceSwipe`、`handleSwipe`、`handleWheel`、`transitionList`、`startTracking` 和 `settleHome`，以及 `Tokens.Motion.slide` 与 `Controls.swift` 的背景色设置路径。

- `space-swipe.ts` 保留4px方向锁定、35%落点、300px/s轻扫、80ms速度采样、250ms滚动轮次间隔、边缘0.5倍宽度上限与0.55阻尼公式。正值表示列表向右移动；DOM 的 `deltaX` 取反后进入同一公式。保留阻尼逆变换，供动画途中接手时恢复手势行程。
- `SpaceDeck.vue` 使用相邻真实列表跟随位移，松手最多进入一个邻居，首尾不循环。返回或进入目标时采样临界阻尼弹簧，并把当前速度交给弹簧；`.15s` 为响应参数，实际动画400ms收敛。Web 平台没有 `CASpringAnimation(perceptualDuration:bounce:)`，采用解析临界阻尼解，内向速度按源码限制在自然频率内，具体弹簧系数是平台适配，不宣称等同 Core Animation 内部系数。
- 点击或快捷键使用源码220ms方向滑动、旧列表180ms淡出；触控板收尾保留两列表不透明。中途开始新手势或再次切换时，读取 WAAPI 当前 presentation transform 后取消旧动画，延续屏幕位置；每个 Space 独立记忆列表滚动位置。后台列表使用 `inert` 与 `aria-hidden`。
- 原生模式先将列表动画交给合成器，再通过已有 `space` 命令切换 Rust 权威状态。`App.vue` 收到新颜色后，按 `ColorView.setColor`/`CALayer.set` 从当前屏幕背景颜色开始，以200ms普通 ease 插值。只处理颜色，不截取整个窗口或冻结网页。
- DOM `WheelEvent` 缺少 `NSEvent.phase`、`momentumPhase` 和 `hasPreciseScrollingDeltas`。像素事件用120ms无输入判定松手，速度取最后一个输入时刻的80ms窗口；250ms内的后续同向尾流归于同次手势，反向输入可接手尚未完成的收尾。行/页单位视为离散滚轮，一轮最多切换一次。这是 Windows 适配启发式，真实触控板驱动仍可能需要调参。
- 减少动态效果时不跟手平移，切换采用原地200ms淡入/120ms淡出，取消的小幅手势保持原列表不动；背景色立即更新。Ctrl/Meta/Alt 修饰滚轮、纵向滚动和已溢出的底部工作区数字栏保留原事件路径，浮层或窗口失焦会取消未完成手势。

## 确认的设计参数

以 `Tokens.swift` 为准：侧栏240（最小216/最大400）；edge6；icon列12/text列36；Sidebar行30/间距2；图标16/间距8；圆角6；13正文/11标签；空间名26 light/字距-.6/代码slot40；Favorites四列/高36/间距6/图标18/圆角8；地址30/圆角8；工具按钮26/图标14；底栏34/数字键18；页面 inset8；命令宽640/field56/row40/最多8行/圆角14/阴影模糊16下移3。

`DESIGN.md` 的空间标题 slot52 与当前 Tokens/布局并不完全一致，采用实际代码40，并保留这个差异。部分旧文档提到 favorites/address 边线，而当前 DESIGN 和 Chrome token 使用软填充；以当前 Chrome 为准。

轻主题 selected=white .82、hover=white .38、field=white .45；深主题 selected=white .18、hover=.05、field=.065。表面浅色 #FFF、深色 #1E1E20，浮层 #FCFCFC/#2A2A2D。快速100ms/选中150ms/标准200ms/空间220ms/大视图300ms；缓动 `cubic-bezier(.23,1,.32,1)`；减少动态效果时取消位移动画。

## 截图与素材校准

已检查：`docs/brand/readme-hero.jpg`、`docs/all-tabs.jpg`、`site/assets/clips/search.jpg`（站内搜索/地址浮层展示）；已定位 `docs/spaces.gif` 与 `site/assets/clips/{spaces,split,desk,peek,reader,search}.{jpg,mp4}`。当前截图与源码年代存在差异，因此尺寸优先采用检出源码，而不照搬旧截图的缩放比例。没有复制任何截图、Logo、应用图标到产品资源。

已访问官方仓库；官网链接的抓取不可用，改为阅读仓库 `site/index.html` 和其中的官方展示素材。另检查了 clips/spaces.jpg 和 clips/peek.jpg。浏览器网页内容来自实际网站，而不是把参考网站画进 Vue。

## 重写与估算

- Windows 增加32 CSS px 自定义标题栏，保留窗口按钮在右侧；按用户后续要求，英文字体改为内嵌 Inter Variable，中文回退 Microsoft YaHei UI，不分发 Apple 字体；1 pt 按1 CSS px 移植，实际物理像素由 DPI 换算。
- 最大化按钮在原生根窗口返回 HTMAXBUTTON，让 Windows 11 提供 Snap Layout 悬停菜单。UI HWND 的对应区域挖孔，由同线程的透明命中绘制子窗口补齐按钮；不跨线程 subclass WebView2 的内部 HWND。按钮几何和颜色由 Vue 上报，宿主处理 DPI、点击和原生绘制。
- Desk 缩略图使用 WebView2 CapturePreview，后台线程缩小并编码为 JPEG，最多480×320；缓存最多64张/8MiB，仅在内存中保存。导航版本校验拒绝过期捕获，未激活页面继续延迟创建。截图和压缩策略是本项目的 Windows 实现。
- 工作区 frame 最终改为 `SpaceColor.tone` 与 OKLab 的逐式 JS 移植，经原有 gamut 回退后输出8位 sRGB；可用 `node scripts/extract-colors.mjs` 重现。默认 sky 为 #D4E4FB/#1D2736，完整结果在 color-palette.json。其余 accent/ink 仍采用共享浅深 tokens，尚未移植完整 Brand.Theme 的逐空间对比度求解；字体宽度、阴影栅格化与图标光学校准属于适配值。
- SVG 采用独立绘制的16单位线图标/1.1 stroke，不复制 SF Symbols；源 `Brand.swift` 的 logo/wordmark 没有移植。
- Windows HWND 网页独立于 CSS，当前打开浮层时隐藏网页 controller，关闭后恢复；Peek 通过 WebView2 CapturePreview 获取内存 PNG 背景，分屏时分别捕获并按各槽位定位。现用 SetWindowRgn 对 wry 公开的子窗口 HWND 做真正裁切：前端从 CSS 的圆角与 border 测量内圆角（当前8减1得到7px），Rust 结合实际子窗口尺寸/DPI 创建区域。分屏标题条遮住顶部圆角，内容只保留底部外角，内部接缝保持直角。
- 默认新标签页 Rove 文字与搜索入口为本项目自有页面；其64号字并非来自原项目，避免被误认为已忠实复刻的设计值。

## 固定标签与 Favorites 状态逻辑移植

除外观参数外，现已迁移 `Tabs/TabManager.swift`、`App/Library.swift`、`Sidebar/SidebarMenus.swift` 与 `Tabs/Tab.swift` 中固定项的核心生命周期。完整映射、兼容迁移和平台适配边界见 [固定标签移植记录](PINNED_TABS_PORT.md)。本地 `TabManager.swift` 的 Git blob 为 `57bdcd12148156ea2daaf2cd77bd5fe0fc99d316`，与本次从 GitHub main 读取的该文件一致。

## 许可证处理与派生逻辑

Tokens 的数值与设计规则转写进入 CSS，已在相关文件标记修改、保留版权及 Apache 来源。`licenses/zigzag-APACHE-2.0.txt`、`zigzag-NOTICE.txt`、`zigzag-TRADEMARKS.md` 原文保留；实际复用范围见 `THIRD_PARTY_NOTICES.md`。未移植 Zigzag 的扩展、密码、Reader 第三方代码。

## Peek 后续移植

按 PeekController.panelFrame 的4pt网格和比例：横向 inset=max(32,snap(width×.08))、纵向 inset=max(24,snap(height×.08×.6))；圆角16、内容 inset4/圆角12、header40、按钮32、图标18，scrim 浅色黑20%/深色42%。后续按用户要求加入链接位置缩放：采用 WAAPI 对 UI 面板和网页快照一起变换，原生 HWND 在静止状态才显示，避免框架和网页错位。入场240ms/离场180ms/初始比例18%为本次交互适配值，缓动沿用已确认的 `.23,1,.32,1`。无链接坐标时使用居中96%起始比例；减少动态效果时取消位移和缩放，仅做80ms淡入淡出。

标准 window.open 的命名目标仅作为链接打开方式提示；普通网页仍然没有内部 IPC 或主机对象。新窗口请求需来自当前标签/分屏/Peek实例并具有用户手势。Peek 与原页面共用 WebView2 站点 profile，但使用独立 controller；不保存临时标签或其历史。晋升把已存在的 controller 移到标签集合，避免重新加载、丢失滚动/DOM状态。

链接主框架把点击位置按视口归一化后放入标准窗口名称；Rust 验证有限值与0…1范围，再结合来源 WebView 的逻辑矩形映射为窗口坐标。子框架采用原生鼠标位置与 DPI 换算回退。动画原点在 viewport 变化时裁到页面边界。尺寸测量使用未变换的布局坐标，不按每个动画帧重排网页。

原生状态为 Opening/Open/Closing，每次关闭递增动画票据。关闭先捕获 Peek 当前内容，Vue 解码后确认，Rust 再隐藏 live controller、允许快照缩放；动画完成后才销毁临时 controller 并恢复原页。离场捕获等待上限为750ms，过期票据/替换后的 id 一律忽略。WAAPI 从当前 presentation 值取消并反向，避免快速关闭、重开时跳帧或误关新页面。
