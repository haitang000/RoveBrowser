# 实际验证记录

## 侧栏宽度调整（Issue #17，2026-11-14）

[Issue #17](https://github.com/haitang000/RoveBrowser/issues/17) 已实现：拖动侧栏右侧 6px 命中区即可改变宽度，拖动期间禁用宽度过渡，用指针捕获保持跟随，并通过现有布局通道同步单页和分屏的真实 WebView2 视口。拖动期间沿用分屏的裁切优化，松手恢复圆角；不显示覆盖层、不重新加载网页。宽度仅在拖动结束后保存至 SQLite，会话重启和侧栏折叠/展开均保留。

宽度限制为 192–480 CSS px；窄窗口临时收窄侧栏，为网页保留至少 240px，窗口变宽后恢复偏好值。旧会话保持原有 240px／窄窗口 216px 默认值。双击边缘或按 Enter 清除偏好；分隔条支持左右方向键、Shift 加速、Home/End 和 Escape 取消当前拖动。取消捕获、窗口失焦、打开浮层及卸载时均结束拖动。保存宽度或标签标题等普通状态广播不再重新启动工作区背景动画。

验证通过：TypeScript／Vite 生产构建、86 项前端单测、85 项串行 Rust 测试、Clippy（`-D warnings`）、Debug 构建，以及 47 项相关 E2E（侧栏宽度、窗口布局、工作区、侧栏滚动、下载和命令面板）。新增 Rust 存储测试覆盖旧会话、重启恢复、非法宽度和重置。同步修正既有测试中的空间名称选择器、外部 favicon 网络依赖，以及原生测试在重新打开固定页、关闭命令面板后缺少等待的问题。

`npm run test:native:sidebar` 在独立临时 profile 中验证真实单页和分屏视口、拖动期间不写入偏好、松手保存、折叠/展开、进程重启和重置，共 6 项检查通过。`npm run test:native` 的 36 项完整原生冒烟检查通过。使用 computer-use 在独立测试窗口中实际拖动右侧边缘，确认侧栏与真实网页重排，保存宽度与桌面显示一致。专项结果位于 `test-results/native-sidebar/result.json`，完整冒烟结果位于 `test-results/native-smoke.json`。CDP 检查不代表桌面合成延迟测量。

## 渐变外框白边与原生圆角（Issue #21，2026-11-14）

[Issue #21](https://github.com/haitang000/RoveBrowser/issues/21) 的白色描边来自原生标题栏绘制路径：外壳 WebView 的四周 6px 缩放区域被裁掉，原生层再用单一主题底色补齐，无法延续工作区的渐变和颗粒。现在 WebView 保持完整背景，原生边缘只以 1/255 的黑色透明度保留输入命中；标题栏图标和悬停色独立合成，关闭按钮悬停仍为不透明红色，避免暗色模式下白色图标与白色悬停使用不同透明度时相互混淆。

Windows 11 使用 `DWMWA_WINDOW_CORNER_PREFERENCE / DWMWCP_ROUND` 请求系统圆角，并用 `DWMWA_BORDER_COLOR / DWMWA_COLOR_NONE` 关闭系统描边。根窗口不再安装 GDI 区域或额外阴影窗口；外壳 CSS 不重复裁切。透明模糊层和标题栏悬停层使用一致的圆角尺寸。Windows 10 或不支持该属性时保留 12px GDI 圆角与兼容阴影，最大化清除区域、还原重新安装。原生方案和兼容条件依据 [Microsoft 圆角说明](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners) 与 [DWM 属性说明](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)。

验证通过：前端生产构建、86 项前端单测、84 项 Rust 测试、Clippy（`-D warnings`）、Debug 构建、10 项窗口布局 E2E 和 36 项完整原生 WebView2 冒烟测试。回归覆盖 100%/125%/150%/200% DPI 的四边四角缩放命中、按钮悬停像素、相同 RGB 的不同透明度、完整 UI 背景、DWM 路径无根窗口区域，以及强制兼容路径的最大化/还原。同步修复了 E2E 所依赖的 JSON 模块导入声明和一个多元素选择器。

computer-use 在 Windows 11 26200、独立临时 profile 中检查了渐变四边、圆角、最大化/还原和深色玻璃画面；测试窗口已退出，用户原有窗口未改动。Windows 10 兼容路径通过真实 HWND/GDI 测试，本轮没有 Windows 10 实机视觉验证。原生冒烟结果位于 `test-results/native-smoke.json`。

## 网站图标清晰度修复（2026-11-12）

用户反馈缓存的网站图标过于模糊。根因是 `native/src/favicon.rs` 把 WebView2 返回的 favicon PNG 交给 `image` crate 的 `thumbnail(64, 64)`：`thumbnail` 不限制放大，16/32px 位图会被插值拉到 64px 再缓存；界面以 16–18px（本机 200% 缩放下 32–36 设备像素）显示时再缩小一次，图标明显发虚。WebView2 自己的 favicon 库只保存 16×16 和 32×32 两种位图，因此把缓存拉到 64 没有任何细节收益。

修复后 `native/src/favicon.rs`：

- 位图只在长边超过 64px 时缩小，绝不放大：16px 图标按 16px 保存，128px 图标缩至 64px（`MAX_SIDE`）。
- 页面加载后由 `ICoreWebView2::ExecuteScript` 读取该页声明的图标链接（`rel` 中的 `icon`、`shortcut icon`、`fluid-icon`、`apple-touch-icon`；`mask-icon` 为单色模板，不采用），按 Zigzag 的排名（SVG → ≥64px 中最小 → ≥32px 中最小 → 其余从大到小）在后台线程用 WinHTTP 取回页面自己的 SVG 或大位图，替换 WebView2 的小位图；SVG 以 `data:image/svg+xml;base64` 保存，页面缩放下不失真。ICO 会解码到自身最大帧。每站点每会话只请求一次，失败后 10 分钟内不重试；请求只带浏览器 UA 与 `Accept`，不携带 Cookie、Referer 或页面脚本，也不向页面注入 IPC。
- 质量不降级：再捕获到更小的 WebView2 位图时不会覆盖已获取的 SVG/大图标；保存项只在图标更清晰、或首页声明、或原本没有图标时更新；同源导航之外，跨站导航会清掉上一站点的旧图标。
- 旧版本写入的 64px 放大模糊图：会话新增可选 `iconVersion` 字段，新版本首次启动时在后台按保存的网址重新获取每个站点的真实图标（页面声明优先，`/favicon.ico`、`/apple-touch-icon.png` 兜底），按批写回 SQLite 并只发布一次状态；没有缓存图标的全新会话不会发起这批请求，迁移标记只在这批请求确实取回图标后写入（离线启动会在下次联网时重试），完成后重启不再重复请求。

验证：`cargo test --manifest-path native/Cargo.toml -- --test-threads=1` 63 项通过（含 12 项图标管道测试：不放大只缩小、SVG 保留/空 SVG 拒绝、ICO 最大帧、声明图标排名、HTML 链接解析、站点只取一次与失败退避、小位图不覆盖清晰图标、首页/深页作用域、旧会话升级），Clippy `-D warnings` 与 Debug 构建通过；`npm run build` 与 70 项前端单元测试通过。

新增 `npm run test:native:favicons`（真实 WebView2 宿主 + 独立临时 profile + 三个本地 fixture 源）7 项检查通过：页面声明的 SVG 取代 WebView2 小位图、同一站点只请求一次并复用、16px 图标按原尺寸缓存、128px 图标缩至 64px、关闭 fixture 服务器后重启进程图标原样恢复、旧会话中 64px 模糊图标启动后按站点真实尺寸重新获取、升级完成后持久化 `iconVersion` 且重启不再重复请求。结果与截图保存在 `test-results/native-favicon/`。既有 `npm run test:native:favorites` 3 项与完整 `npm run test:native` 原生冒烟（35 项，含图标随 SQLite 重启恢复）通过。

实机核对：把当前用户会话（43 个旧缓存图标）复制到临时用户目录，用新构建启动并截图。44 个图标中 40–42 个被替换（网络波动影响个别站点），全部为 SVG 或 ≥32px 位图（其中 22–24 个为 SVG），迁移完成后 `iconVersion` 写回 SQLite；侧栏与顶部常用网站在 200% 缩放下明显清晰，旧缓存与修复后的对照图保存在 `.local/favicon-proof/before-after.png` 与 `.local/favicon-proof/after-sidebar.png`。

局限：少数站点（实测 grok.com、mail.126.com）会拦截非页面发起的图标请求或依赖登录，这些站点保留旧缓存直到正常访问页面重新捕获；图标请求走 WinHTTP 系统代理且不带页面 Cookie，需要登录才返回图标的站点会回退到 WebView2 的位图。混合 DPI 多屏与系统代理异常场景未逐一实机覆盖。

## 0.1.0-beta5：安装程序回退原版向导

2026-11-10：安装程序改回 Inno Setup 原版向导外观，移除 beta4 的自定义 Rove 官网风格界面（自绘侧栏、标题、按钮、进度条，安装进程临时加载的 Outfit 字体及其许可证）；安装目录、开始菜单／可选桌面快捷方式、覆盖升级、卸载、静默安装与 WebView2 运行时补装行为保持不变。`installer/` 只保留 `Rove.iss`、中文语言文件与版本定义。安装包体积由 beta4 的约 6.03 MB 减至约 6.0 MB。

验证：`npm run build`（vue-tsc + Vite 生产构建）通过；59 项前端单元测试、43 项 Rust 测试（`--test-threads=1`）与 Clippy（`-D warnings`）通过。`npm run installer` 使用 Inno Setup 6.7.3 完成前端、x64 MSVC Release（静态链接 CRT）和安装器编译。实机截图核对英文与简体中文向导均为 Inno Setup 标准现代向导外观，未使用自绘控件；语言文件均能编译并按系统语言显示。

静默安装冒烟：使用独立临时安装路径与临时用户目录，注册显示名 `Rove 0.1.0-beta5`、版本 `0.1.0-beta5`；安装的 `Rove.exe` 与 x64 Release 构建 SHA-256 一致；创建开始菜单快捷方式，静默安装不创建桌面快捷方式；已有 WebView2 时跳过引导程序。安装版以隔离用户目录启动并创建 `era.sqlite`。卸载移除应用、注册信息与开始菜单快捷方式，同时保留临时数据库。另验证 beta4 → beta5 覆盖升级后注册版本与二进制更新到 beta5，卸载同样保留数据。结果保存在 `.local/beta5-installer-smoke*/`、`.local/beta5-upgrade-smoke/`、`.local/installer-stock-wizard-*.png` 与安装日志中。

本轮未重跑界面 E2E 与原生 WebView2 功能集成（本次改动仅涉及安装程序与版本文档）；未在缺少 WebView2 的干净系统上重复运行时补装，也未实机覆盖 ARM64、多屏混合 DPI 或系统高对比度模式。安装程序尚未代码签名。

## 0.1.0-beta4：Windows 安装器

2026-11-08：新增当前用户安装器及 GitHub Release 自动打包，默认安装到 `%LOCALAPPDATA%\Programs\Rove`，支持开始菜单／可选桌面快捷方式、覆盖升级、卸载与静默安装。安装器沿用 Rove 官网的白底、深灰标题、Outfit 字体和深色渐变主按钮；字体仅为当前安装进程临时加载，高对比度模式保留系统向导。发布应用静态链接 VC++ 运行库，安装器校验微软 WebView2 Evergreen 引导程序的数字签名，仅在运行时缺失时调用它。

验证：`npm run installer` 完成 TypeScript 检查、Vite 生产构建、x64 MSVC Release 构建和 Inno Setup 6.7.3 编译；54 项前端单元测试、Clippy（`-D warnings`）通过。38 项 Rust 测试以 `--test-threads=1` 全部通过；默认并行模式下两个真实 HWND 区域／命中测试曾失败，串行复测通过。界面 E2E 全量 63 项中 62 项首次通过，真实水平滚轮用例读取中间状态时已进入收尾阶段；工作区专项 19 项以单 worker 复测全部通过。

本机使用独立安装路径和临时用户目录验证 beta3 → beta4 覆盖升级、版本注册、安装文件与发布二进制的 SHA-256 一致、中文路径启动及 SQLite 创建；卸载移除应用和注册信息，同时保留临时数据库与预置标记。另已验证桌面／开始菜单快捷方式及目标路径、已有 WebView2 时跳过引导程序、中英文安装文案编译和中文向导实机显示。结果保存在 `.local/beta4-release-smoke-*/checks.json` 及安装日志中。

未在缺少 WebView2 的干净系统上执行运行时补装，也未实机覆盖 ARM64、多屏混合 DPI 或系统高对比度模式。本轮未重跑全部原生 WebView2 功能集成；设置功能的已有验证见 `docs/SETTINGS_PORT.md`。安装程序尚未代码签名。

## 0.1.0-beta3：独立透明弹窗层

2026-11-08：浏览器外壳与弹窗使用两个受信 WebView。命令栏、设置、全部标签页、右键菜单及网址编辑、标签悬停预览、Peek 外壳和错误提示统一放在透明弹窗 WebView，保持在真实网页之上；原生窗口区域仅保留当前弹窗，空区域透传鼠标，Peek 内容区域挖孔后交给真实临时网页。移除命令栏、悬停卡片和 Peek 来源页的整页背景捕获与解码握手。命令栏恢复淡入与轻微缩放，底层单页、分屏及 Peek 持续可见，不替换为静态来源截图。

验证通过：前端生产构建、46 项前端单测、33 项 Rust 测试、Clippy（`-D warnings`）、Windows Release 构建。真实 HWND 测试覆盖弹窗区域并集、空区域和圆角 Peek 挖孔在 1×/1.5×/2× DPI 的结果。51 项界面用例分次通过；首次并行全量运行有菜单 Esc 焦点回归及一项手势时序失败，修复菜单焦点后相关 9 项和弹窗相关 17 项复测通过，手势用例单独复测通过。

真实 WebView2 验证：完整原生冒烟 35 项、弹窗专项 6 项、悬停预览专项 9 项及网站缓存专项 3 项通过。覆盖来源页 DOM/滚动/可见性、入场动画、快速开关、分屏及 Peek 叠加、关闭焦点恢复、设置/菜单/编辑/错误提示归属、预览刷新/淡入淡出，以及离线重启缓存。结果在 `test-results/native-smoke.json`、`test-results/native-command/`、`test-results/native-tab-preview/` 和 `test-results/native-favorites/`。均使用临时用户目录。

Peek 自身的离场缩放仍需捕获其临时网页；捕获超时会使用主题占位完成动画，来源网页继续显示。早期全量检查曾因要求每次都有 Peek 截图而失败，脚本现明确分别验证截图与占位两条路径；最后一次全量检查成功捕获截图并通过。桌面实机检查被用户物理 Esc 中止，未将这次未完成的检查作为验证证据；CDP 结果不等同于逐帧桌面录像。

以下按功能保留历史验证记录；背景快照和隐藏网页的旧实现说明已由本节取代。

## 命令栏在当前网页上展开

命令栏现在位于 `.page-area` 内，按网页区域定位、限制宽高，侧栏收起或最小窗口下仍保持在网页范围内。原生宿主通过内存快照显示单页、分屏及 Peek 的背景；全部图片解码并经过绘制帧后才抬高 UI 层。底下的 WebView2 保持可见渲染，关闭时先恢复原生层级，再释放背景图片，并将焦点返回当前网页或 Peek。命令栏展开期间显示打开时的静态快照；图片不写入历史、会话或磁盘。

本次验证：`npm run build`、22 项前端单元测试、40 项完整界面端到端测试、22 项 Rust 测试及 Clippy（`-D warnings`）通过。`npm run test:native:command` 的 4 项原生专项通过，覆盖真实网页像素、DOM/滚动/可见性与焦点、分屏和 Peek、延迟解码、快速关闭重开时拒绝旧回调、坏图片恢复，以及 Ctrl+T 取消后不创建空白标签。

原生专项使用独立临时 profile 和 19646/19647 调试端口，结果及截图位于 `.local/command-native-results/focused/`。已有 Debug 可执行文件运行中，因此使用 `.local/command-build` 源码快照和独立 crate 名构建，未关闭原有窗口。完整原生 smoke 曾在命令栏检查之前的固定页返回起始网址步骤超时；以上通过结论来自独立的命令栏专项，不表示完整原生 smoke 全部通过。CDP 截图也不等同于桌面逐帧录像。

日期：2026-11-07。Windows 本机，MSVC Build Tools 18，Rust 1.99.1，Node 24.18.0，WebView2 Evergreen 156/157 已安装。没有使用 Electron、完整 Chromium 源码或 iframe。

| 检查 | 实际结果 |
|---|---|
| 上游检出 | `git clone --depth 1` 成功；提交 `5338e99f58aa9a75a0696d5d96ec74353c3b3c7d` |
| Vue/TypeScript | `npm run build` 成功，vue-tsc 严格类型检查 + Vite 生产构建 |
| 前端单元测试 | `npm test`：22/22，覆盖 URL、标签/固定项生命周期、工作区滑动、Peek几何等 |
| 界面端到端 | 完整回归37项通过；新增固定分屏用例后固定标签专项5/5通过，后续复测与偶发失败记录见下文 |
| Rust 单元/存储测试 | `cargo test --manifest-path native/Cargo.toml`：22/22，含真实 HWND/GDI、固定项卸载/迁移/控制器身份、分屏关系及SQLite |
| Rust 静态检查 | `cargo clippy --manifest-path native/Cargo.toml -- -D warnings` 通过 |
| Debug 构建 | `cargo build --manifest-path native/Cargo.toml` 成功 |
| Release 构建 | `cargo build --release --manifest-path native/Cargo.toml` 成功；exe 约4.33 MiB（含本地 Inter 字体和图片编解码，不含系统 WebView2 Runtime） |
| 原生集成 | `npm run test:native`：31项通过；本地固定页面 + 实际 Windows 宿主/WebView2，含固定项跨工作区复用、跨站/登录导航、控制器身份及会话恢复 |
| Windows 实机 | 使用 computer-use/sky 打开 Debug 和 Release；加载外部 example.com 与 github.com，观察实际网页文档和标题同步，网页与240宽侧栏边界分离；从网页焦点触发 Ctrl+L 打开受信 UI；Release 最大化后布局正确 |
| 来源/许可 | 保存上游 LICENSE/NOTICE/商标说明；提取162项锁定依赖记录（含 Rust 构建依赖）及256份可用许可/notice文本，包含 Inter 的 OFL 和 image 编解码依赖 |

原生集成8项：Inter 字体实际加载；真实页面加载/标题；网页 `window.ipc` 不存在且伪造 WebMessage 不执行内部命令；前进/后退；UI Ctrl+L/浮层恢复；双独立 WebView 分屏；正常退出并重启后恢复标签、固定和分屏关系；SQLite 历史检索。使用临时 profile，不触碰用户已有浏览数据。执行后 `test-results/native-smoke.json` 保存结果和临时目录。

## Zigzag 固定标签核心逻辑迁移验证

本次实际阅读 Swift `Library`、`TabManager`、`SidebarMenus` 和 `Tab` 的固定页规则，将保存项与运行中页面分离；源码映射及未覆盖项详见 [固定标签移植记录](PINNED_TABS_PORT.md)。

- `npm test`：22项通过；Rust状态/存储及平台测试22项通过；`cargo clippy ... -- -D warnings`通过。
- 完整界面回归37项通过；随后新增固定分屏专项检查，固定标签专项5项均通过，覆盖卸载后重开、名称/网址独立编辑、跨类型拖动、Favorites移除后保留页面及固定分屏补回已关闭的一侧。后续15项复测中14项通过，既有标签离场动画的宽度断言曾读到0，单独复测通过；未改动动画实现。
- `.local/bookmark-native-results/native-smoke.json`记录31项原生集成检查通过：真实WebView2中转换普通标签/固定项/Favorites以及跨工作区切换，DOM标记不丢失；跨站链接保留来源固定页，登录链接留在原控制器，返回保存网址正常；SQLite重启恢复检查通过。全部使用临时profile，没有修改日常用户目录。
- 原生Peek快照检查曾两次超时，诊断复测最终完整通过；捕获仍沿用750ms等待和失败回退策略，本次未修改该策略。自动验证不能保证所有机器上每次捕获均及时成功。

## 顶部置顶网站

移除顶部网站管理弹窗、相关入口及侧栏“常用网站”文字标题。通过普通标签或固定标签的右键菜单、拖拽添加；通过网站右键菜单编辑和移除，拖拽排序。前端与 Rust 均限制最多 8 个网站，分屏组合按两个网站计数，已有项目在满额时仍能排序。旧会话超过上限的项目移至当前工作区的固定标签，分屏组合整体迁移。

图标使用 WebView2 GetFavicon 获取 PNG，限制输入大小和解码尺寸，只缩小不放大；页面声明的 SVG／大图标另行在后台取回（见[网站图标清晰度修复](#网站图标清晰度修复2026-11-12)，旧版本缓存的放大模糊图标会在首次启动时自动重新获取）；随保存项写入 SQLite。卸载和重新打开时使用保存图标，跨域修改保存网址时清理旧图标；晚于导航或卸载返回的旧图标回调会被忽略。同源普通标签加载图标也会填充已有置顶网站缓存。

回归覆盖八个上限、普通/固定/分屏添加的原子性、旧会话迁移、清空后通过普通标签重新添加、右键编辑、拖拽排序、离线图标显示以及 SQLite 重启恢复。真实宿主脚本使用本地 PNG favicon 验证 WebView2 获取、卸载后显示与进程重启后的缓存。

本次前端构建通过，46 项前端单元测试、33 项 Rust 测试通过；网站、固定标签与窗口布局的 13 项相关端到端测试分次通过。`test:native:favorites` 在独立临时目录通过真实 WebView2 缓存及服务器关闭后的进程重启验证。全量原生冒烟在后续 Sign in 链接的可见性等待处超时，未完成全量验证。

## Peek 动画闪白修复

根因是异步背景 PNG 尚未解码、绘制，原生网页便已隐藏；离场时也存在先移除快照再恢复原生网页的空档。现在原网页保持可见渲染，全部分屏背景就绪、解码并经过绘制帧后，才通过同一父窗口内的 HWND 层级切换显示 UI 动画层。关闭时先露出原网页，再移除快照，不再隐藏/恢复来源 WebView。重复打开时一次性发布已有背景，避免先清空旧背景。捕获超时或解码失败时取消本次预览并保留原网页。

离场仍先等待 Peek 页面快照解码、绘制，再隐藏临时控制器；等待捕获的上限由120ms改为750ms，避免忙碌渲染器过早退回占位画面。临时控制器仍在动画完成后释放。层级切换仅限本窗口的子 HWND，不使用全局置顶。API 依据：[SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)。

本次专项验证：5项 Peek 动效端到端全部通过，含故意延迟双背景解码、坏图片取消、动画反向及减少动态效果；独立端口运行完整界面测试11项通过。原生集成25项通过，确认来源网页在入场、预览和离场阶段的 `document.visibilityState` 持续为 visible，并保留 DOM/滚动/晋升行为。使用 computer-use 查看真实窗口的延迟解码等待、离场中间帧和结束画面，三处均保持高对比网页背景，没有露出白色底层。自动测试不是逐帧桌面录像，未宣称覆盖所有显卡/DPI/页面。

并行开发时使用5187端口与独立原生调试端口19446，结果在 `.local/peek-native-results/native-smoke.json`。为避免另一任务正在使用主 Debug 可执行文件，原生测试与 Release 打包使用 `.local/peek-flash-build` 的源码快照，以独立 crate 名复用已编译依赖；窗口名称、用户数据位置和网页 IPC 边界保持原有行为。Rust测试14项、Clippy（-D warnings）和 Release 构建通过。

## 视觉校准

源截图：readme-hero、all-tabs、clips/search、spaces、peek。当前浅/深/命令面板截图由测试保存到 `test-results/ui-{light,dark,command}.png`。检验了源码尺寸、色彩、圆角、选中填充和图标对齐；frame 色彩由原始 OKLab 公式生成。Windows 字体和独立 HWND 网页裁切仍与 AppKit 存在差异，没有声称像素级一致。

## 验证中发现并修复

- SQLite 查询临时借用生命周期错误，已修复并通过 Rust 构建。
- Ctrl+Shift+反斜线产生 `|` 的键值，改为根据 `KeyboardEvent.code` 识别。
- Vite 监听 `native/target` 中链接中的 DLL 导致 EBUSY；已忽略 native、reference、licenses、测试目录，重测通过。
- 最小窗口下命令结果列表超高，改为按窗口剩余高度裁切并滚动，补充端到端边界验证。
- 固定分屏已在固定标签中时不再反向切换为取消固定。
- 测试脚本重启时曾连接旧 WebView2 调试 endpoint；改为正常退出并等待渲染器目标消失，再等待新目标后连接，重测通过。
- CDP 合成键盘事件不会触发 Windows 原生 AcceleratorKeyPressed；该原生路径另用 Windows 实机输入核验，未把合成键盘检查当作该路径证据。

## 覆盖限制

尚未完成标签悬停预览、触控板惯性空间切换、浮动侧栏边缘呼出。分屏比例不跨重启保存。Peek 与命令栏已提供背景快照，设置、全部标签页等其他浮层仍隐藏网页而不保留页面快照背景。

## 真实缩略图与原生 Snap 菜单

Desk 卡片使用 WebView2 CapturePreview 获取实际页面，再在工作线程按比例缩至480×320以内，编码为质量75的 JPEG。原生集成检查图片可解码、尺寸受限，且实际像素与蓝色本地 fixture 相符；不是仅检查图片标签或 data URL。结果和截图保留在 `.local/snap-native-results/native-smoke.json`、`native-thumbnails.png`，27项完整检查通过。

缩略图仅存在内存，最多64张、8MiB；导航版本变化后拒绝旧异步结果，关闭标签后清理缓存，不写入SQLite或用户历史。尚未创建的网页不会为了截图自动加载，失败时显示占位。Peek 的全尺寸捕获与动画优先，避免后台缩略图占用同一个渲染器的捕获流程。

最大化按钮通过根窗口 WM_NCHITTEST 返回 HTMAXBUTTON，由 Windows 11 显示原生 Snap Layout 菜单。真实 HWND 测试覆盖负屏幕坐标、1×/1.5×/2× DPI、UI区域挖孔，以及不抢占相邻最小化/关闭按钮。computer-use 实机观察到了系统提供的六种布局悬停菜单；实际点击最大化到屏幕工作区后，再点击可还原，按钮首帧及主题颜色正常。最小窗口宽度改为480px，并通过界面尺寸检查。

菜单是否启用由系统设置决定；Windows 10 保留普通最大化。尚未实机覆盖系统菜单中每个布局区域的选择、多屏混合 DPI 和所有 Windows 版本。参考：[Microsoft 自定义标题栏接入 Snap Layout](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu)。

## 原生圆角与字体后续验证

`native/src/clip.rs` 使用 [SetWindowRgn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn) 限制实际网页窗口的绘制区域。区域成功安装后交给 Windows 管理，创建失败或未安装的临时 GDI 对象由 RAII 清理；仅尺寸/圆角变化时重建。未向普通网页添加任何新 IPC 或脚本。

使用两个高对比纯色的本地真实网页检查了单页四角、双实例分屏底部外角、顶部标题条直角接缝，以及最大化后的边界。字体从自定义协议提供 `font/woff2` 资源，原生集成测试确认 FontFace 状态为 loaded；无需外部字体服务。

尚未覆盖多屏混合 DPI、全部 Windows 10/11 版本、系统 Snap 的完整行为、音视频会议、复杂登录/弹窗及大规模长时间使用。窗口拖动与缩放后续改为 WebView2 原生拖动区域和 Win32 命中测试，见下方修复记录；上述未覆盖项不作为通过声明。所有剩余平台差异和功能范围也在 README 中列出。

## Peek 后续实现与验证

已实际移植 `PeekController.swift` 的面板比例、40px标题区、16/12px圆角、4px内容内边距、32px按钮及20%/42%遮罩；不复制原品牌。Peek 使用独立 WebView2，创建、替换、关闭、晋升均由 Rust 管理。网页侧没有 IPC handler 或 host objects；Shift 点击由无特权链接适配脚本变成标准命名新窗口请求，原生端验证用户手势、来源及 HTTP/HTTPS 地址。

测试新增 `tests/peek-layout.test.ts`、`tests/e2e/peek.spec.ts` 并扩展 `scripts/native-smoke.mjs`。当前前端单元测试7项、界面端到端4项、Rust测试10项，原生集成16项通过；TypeScript/Vite、Clippy（-D warnings）与 Debug/Release 构建通过。

原生证据包括：真实 Shift 点击普通和 target=_blank 链接；不增加标签的临时预览；原标签 DOM 不变；预览导航/后退独立；晋升后 JS marker、滚动位置和导航栈保留，服务器访问次数没有因晋升增加；连续 Shift 点击释放旧控制器；关闭按钮、遮罩和 UI Esc；禁止 file/internal scheme；分屏两个原生页面都捕获背景；重启只恢复原标签/已晋升页面，临时 Peek 不进入会话。截图只保留测试界面，临时页面快照在运行中只存在内存，不写用户历史或外部服务。

原生 Peek 页面焦点下的 Escape 由已有 WebView2 AcceleratorKeyPressed 扩展处理；CDP 合成键盘不能覆盖 Win32 accelerator 路径，当前新增自动测试覆盖的是 UI 焦点下 Escape。背景捕获失败或单张超过8MiB时取消本次预览并保留原网页，避免显示空白背景。

## 链接原点动画后续验证

`tests/e2e/peek-motion.spec.ts` 暂停真实 WAAPI 时间线，逐帧核对18%初始卡片的中心等于链接点击位置；放大结束后回到布局矩形，关闭收缩到同一个坐标。另验证入场65ms时直接关闭不会跳回初始位置，旧预览的完成消息不能关闭新预览；系统 reduce 设置下仅淡入淡出，不发生 translate/scale。

`scripts/native-smoke.mjs` 使用真实 Shift 点击，核对 Rust 发布的窗口坐标与来源 WebView/链接 DOM 坐标一致；等待 entry 完成后才使用网页。离场时暂停动画，确认网页 PNG 已解码、原生 controller 尚未销毁，缩放的快照包含真实网页内容；完成动画后才观察到 controller 关闭。还验证快速关闭/重开和旧票据的拒绝。过程截图为 `test-results/peek-enter-origin.png` 和 `peek-motion-native-exit.png`。

实现依据：[WAAPI commitStyles](https://developer.mozilla.org/en-US/docs/Web/API/Animation/commitStyles)、[WebView2 CapturePreview](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2?view=webview2-1.0.3595.46)。没有为这些动效新增前端动画依赖，也未向普通网页增加内部 IPC。

## 窗口整体圆角

根 HWND 按窗口完整边界安装 12px 逻辑半径的区域，随尺寸和 DPI 更新；最大化移除区域，还原后重新安装。界面外层同步圆角并裁切标题栏和浮层，独立预览采用相同默认半径。

实际 HWND 测试覆盖 1×/1.5× 半径、重复移除以及相同尺寸下的恢复；原生集成新增最大化/还原检查。前端构建、7项界面端到端、11项 Rust 测试、Clippy 和 Release 构建通过，原生集成最终20项通过。原生集成期间 Peek 离场快照检查曾两次超时，重跑通过；该间歇性问题未纳入此次圆角修改。使用 computer-use 查看实际窗口及最大化操作，测试使用独立临时数据目录。

## 启动闪白与侧栏尺寸更新

原生 UI 在初始化脚本中获得保存的会话，首屏直接使用对应主题和折叠状态；UI 与网页 WebView2 的默认底色匹配主题表面。Vue 挂载、字体加载及两个动画帧完成后发送 `presented`，宿主再显示窗口，避免先显示空白 UI 或预览会话。

网页槽位在 ResizeObserver 回调内直接上报当前布局，移除额外一帧的 requestAnimationFrame 调度；Vue 只监听实际影响几何的状态，并在 DOM 更新后测量。Rust 的 layout 命令直接更新现有控制器的几何，不再执行页面创建、导航和主题设置；可见性、主题也只在状态变化时调用原生 API。

`tests/e2e/window-layout.spec.ts` 验证首屏深色和折叠布局，并在侧栏收起、展开及中途反向时，对比分屏两侧每个 ResizeObserver 绘制帧的上报宽度与 DOM 宽度（误差小于 0.1 CSS px）。原生集成暂停过渡并检查四个中间位置，真实单页/分屏 viewport 与槽位宽度误差不超过 1 CSS px，期间没有页面 visibilitychange。重启检查保存的深色/折叠状态，并通过进程的可见主窗口句柄确认窗口实际显示。测试使用临时 profile。

本次最终验证：前端构建、9 项前端单元测试、`npx playwright test --workers=1` 的 11 项 UI 测试、14 项 Rust 测试、Clippy（`-D warnings`）、Debug/Release 构建及 25 项原生集成检查全部通过。并行运行 UI 测试时曾出现 Peek 动画时序用例失败，串行全套通过；未将 CDP 检查等同于屏幕录像或多屏 DPI 验证。

## Space 触控板切换滞后（Issue #9，2026-11-08）

原逻辑持续重置120ms松手计时器，触控板惯性尾流会推迟Space状态提交；结束后250ms内的同向新轻扫也会被当作尾流吞掉。最终实现按前后短窗口的速度衰减提前收尾，拖满一页立即提交；无输入等待按事件频率调整为32–64ms，保留250ms尾流归并。明显的新加速或短暂停顿后的同等强度新输入可立即接手动画。提前收尾使用最新逻辑位移，避免Vue尚未绘制最后一包输入时产生位置回跳。

首轮修复遗漏小幅回退及首尾回弹，仍受惯性尾流拖延；严格要求连续降速也会漏掉带抖动的尾流。新增回归复现这两类问题以及无尾流时固定80ms等待，最终统一所有收尾分支，并用短窗口比较容忍噪声。首尾弹簧继承阻尼后的列表实际速度，WAAPI显式锚定当前时间轴，避免额外等待启动帧。

50项单元测试、19项Space端到端测试、TypeScript/Vite和Windows Debug构建通过。新增覆盖边界、小幅及带抖动尾流期间的可见回归、无尾流快速启动、短暂停顿后的同速手势接手；既有不多跳、连续轻扫、最后一包位移、取消、首尾阻尼、动画中断、滚动位置、纵向滚动及减少动态效果继续通过。Space测试屏蔽外部favicon请求，避免无关网络加载导致页面初始化超时。

`npm run test:native:spaces` 的6项WebView2检查通过，逐帧记录实际transform及动画时间：无尾流的实测启动时间约35ms，首次位移采样约62ms时已从40px回到约29px；边界、小幅及抖动尾流的14包输入尚未结束时，列表已经明显回归。另确认Space IPC更新Rust权威状态和颜色且尾流不多跳、双向切换保留网页DOM及滚动位置且不重新加载、SQLite保存及重启恢复、减少动态效果不平移列表。结果和逐帧数据在 `test-results/native-spaces/result.json`，测试进程已退出。输入由自动化滚轮事件模拟，尚未覆盖不同实体触控板驱动。

## 侧栏 Space 轻扫与背景过渡

移植基线仍为 `5338e99f58aa9a75a0696d5d96ec74353c3b3c7d`。`tests/space-swipe.test.ts` 覆盖原 Swift 的方向/落点、速度采样、首尾阻尼及逆变换，以及携带速度且无过冲的临界阻尼收尾；当前22项单元测试通过。TypeScript/Vite 与 Windows Debug/Release 构建通过。

新增 `tests/e2e/spaces.spec.ts` 的11项 Edge 用例全部通过：真实横向滚轮输入、相邻真实列表与数字键跟手进度、惯性尾流不连续跳空间、小幅回退/轻扫落点/反向取消、纵向锁定与 Ctrl 滚轮、首尾阻尼、离散横向滚轮、动画中断时的位置连续、点击/键盘方向与背景颜色连续、各空间滚动位置、溢出的数字栏、减少动态效果下的原地切换与取消、浮层取消，以及同色状态广播不会重启背景动画。

UI 共覆盖37项，最终全量执行中36项首轮通过。书签拖放用例曾在连续拖动时超时；原检查通过角色查询认为旧行已消失，此时该行仅被标记 inert，离场动画仍保留实际 DOM。补上等待源行实际移除后，该用例连续5次复验通过。测试使用独立5179端口与输出目录，避免和其他开发会话的 Vite/测试清理相互影响。

新增 `scripts/native-spaces-smoke.mjs`，可在 `npm run build` 与 `cargo build --manifest-path native/Cargo.toml` 后运行 `npm run test:native:spaces`。它使用独立临时用户目录、两个本地固定网页和19546/19547调试端口，4项原生检查通过：真实 UI WebView2 横向滚轮经 Space IPC 切换 Rust 状态与颜色；双向切换保留两个页面的 DOM 标记与滚动位置、服务器没有重复访问；SQLite 保存滑动落点并在重启首屏恢复；原生 UI 的减少动态效果模式不产生列表位移。测试关闭自身启动的进程。

上述验证覆盖浏览器/原生 WebView2 实际动画与事件路径，没有使用物理触控板测量驱动的松手/惯性手感。DOM 无 macOS 手势阶段信号，120ms松手与250ms轮次间隔是平台适配值，仍需不同触控板硬件覆盖。具体源码映射和参数见 `SOURCE_ANALYSIS.md`。

## 窗口拖动与自由缩放修复

旧实现通过 DOM pointerdown → IPC → winit 启动移动/缩放；实际窗口复现标题栏拖动无效。当前改用 WebView2 原生 `app-region: drag` 标题栏，按钮区域为 `no-drag`，支持系统移动、双击最大化/还原及右键系统菜单。移除 DOM 缩放覆盖层和对应拖动 IPC。

`caption.rs` 将 UI HWND 的四边与最大化按钮区域让给同线程原生绘制子窗口，子窗口返回 `HTTRANSPARENT`，根窗口直接接收非客户区输入。四边宽6逻辑像素，角落命中范围12逻辑像素；按 DPI 换算物理坐标，最大化时取消缩放边框，还原后恢复。GDI 临时区域采用 RAII 管理，成功安装后将所有权交给 Windows。

真实 HWND 回归检查覆盖 DOM 上报前的边框、八个缩放方向、负屏幕坐标、1×/1.5×/2× DPI、内部网页及相邻窗口按钮不被覆盖，以及相同尺寸下最大化/还原区域更新。computer-use 使用独立测试 profile 实测标题栏连续移动、上/左边缘缩放、右下角双向缩放、480×400最小尺寸、双击最大化、原生按钮还原、还原后继续缩放和系统 Snap 悬停菜单。

前端构建、22项前端单元测试、22项 Rust 测试、Clippy（`-D warnings`）、Debug/Release 构建、5项窗口布局及基础交互 UI 测试、35项原生集成检查通过。原生结果保留在 `.local/window-fix/native-results/native-smoke.json`；测试进程已退出，用户原有运行窗口未关闭。多屏混合 DPI 和其他 Windows 版本仍需实机覆盖。

接口依据：[WebView2 原生拖动区域](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2settings9)、[Win32 WM_NCHITTEST](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest)。

## 整个窗口的圆角投影

`native/src/shadow.rs` 为保留12px原生圆角的主窗口增加独立透明投影层。主窗口的 `SetWindowRgn` 裁切会阻止普通 DWM 阴影显示，因此投影由后方的 layered tool window 绘制：黑色预乘 BGRA，12px柔和衰减、6px向下偏移与较窄的接触阴影，保留48px透明外缘。随窗口尺寸和 DPI 重绘；单纯移动复用已上传的位图，仅同步位置。

`caption.rs` 在原生位置、显示、尺寸和激活消息中同步阴影。投影层使用 `WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`，始终位于主窗口后方；不拦截鼠标、不抢焦点、不出现在任务栏。主窗口隐藏、最小化或最大化时隐藏投影，还原后恢复；退出时由 RAII 销毁窗口和临时 GDI 资源。

新增像素回归检查覆盖四边衰减、圆角、内部透明、外缘归零和1×/1.5×/2× DPI。23项 Rust 测试、Clippy（`-D warnings`）、Debug/Release 构建通过。computer-use 在独立 profile 和本地对比背景下观察了真实窗口外侧的四边投影，并实测缩放、移动、最大化/还原、最小化/恢复。35项原生集成检查最终通过，结果位于 `.local/shadow-review/native-retry/native-smoke.json`；首轮既有 Peek 离场动画的 `.peek-surface` 检查超时，重跑通过，原失败证据保留在 `.local/shadow-review/native-results`。

接口依据：[UpdateLayeredWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow)、[Layered Windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows)。尚未覆盖多屏混合 DPI 和其他 Windows 版本。

## 标签悬停预览（2026-11-08）

新增 `tests/tab-preview.test.ts` 验证当前分屏排除、部分卸载的保存组合、运行网址与保存标题、工作区范围，以及窄窗口/上下边缘定位。前端合计43项单元测试、TypeScript/Vite构建通过。新增5项 `tests/e2e/tab-preview.spec.ts`，覆盖短暂停留取消、600ms展开、相邻项立即更新、点击/滚动/Esc/浮层取消、未加载常用网站占位、分屏图片更新、480×400窗口、深色/减少动态效果，以及原生关闭回执与过期票据处理；5项全部通过。新增用例屏蔽外部 favicon 请求，使用显式图片 fixture 与图标回退，不依赖外网时序。

全量46项 Edge UI 回归首次执行43项通过，3项在 `beforeEach` 的 `page.goto(..., waitUntil: load)` 超时，尚未进入功能断言。为排除外部图标请求，在 `.local/tab-preview-review/offline.playwright.config.ts` 使用独立5182端口，并令非本地代理请求快速失败，原3项（工作区/深色/分屏、纵向手势锁定、动画中途接手）均通过；该配置下新增5项也通过。保留原始失败记录于 `test-results`，没有修改这3项既有测试或应用的联网行为。

Rust合计27项测试、Clippy（`-D warnings`）及Debug构建通过。GDI区域测试新增预览差集、1×/1.5×/2× DPI、负局部坐标，以及真实 HWND 在同尺寸下反复露出/恢复预览区域，确认其他像素与原网页圆角保留，区域缓存不会遗漏恢复。

`npm run test:native:preview` 使用独立临时profile、本地固定网页与19746/19747端口，8项检查通过：真实单页快照/标题/URL、按秒更新、当前页持续可见与DOM保持、关闭后按钮仍可操作、未打开项没有网页请求或新增标签、从分屏任意一半查看两张快照、两侧原生网页保持可见、主题/Esc/命令栏协作。另执行既有命令栏4项及工作区4项原生检查，均通过。结果副本保留在 `.local/tab-preview-review/{native-tab-preview-smoke,native-command-smoke,native-spaces-smoke}.json`。

computer-use 在独立测试窗口中核对了深色预览覆盖真实分屏的画面，周围两页正常显示，卡片圆角及边线正确。测试进程均已退出。原生版通过 HWND 区域差集展示卡片，不冻结整个网页；没有复制原版卡片外阴影和位置动画。快照复用原有64张/8MiB内存缓存，不写入会话或磁盘，不为占位项创建控制器。

本次原生启动还复现并修复了 document-created 初始化时访问尚未存在的 `documentElement` 而提前异常的问题。宿主标识和初始状态现在先于Vue挂载设置，DOM属性在挂载后更新；原生检查明确断言宿主模式、初始状态和DOM标识均正确。

## 基本布局回退（2026-11-08）

按用户要求恢复 `db0e5f9` 时期的基本结构：完整32px顶栏，Rove标题位于窗口左上方12px，下方是240px侧栏与内缩圆角网页卡片。移除将标题栏推至侧栏右侧、网页贴边和侧栏占满窗口高度的CSS及无边框页面设置入口；已有会话的 `borderless` 字段保留兼容，但不再驱动实际布局。玻璃外观、分屏、命令栏和悬停预览继续工作。

前端构建通过。相关14项界面回归通过，覆盖480px窄窗口、分屏、命令栏、全部悬停预览和侧栏开合；新增回归明确使用 `borderless: true` 的旧会话，断言标题栏 `(0,0,1280,32)`、Rove标题 `x=12`、侧栏 `(0,32,240,788)`、页面 `(240,32,1032,780)`，新标签内容在网页卡片内居中。修复版原生8项悬停检查通过，并通过computer-use核对真实窗口的完整标题栏、左上角Rove和下方侧栏/网页结构。

用户正在运行的 `native/target/debug/rove.exe` 被Windows锁定，生成独立的 `native/target/debug/rove-layout-restored.exe` 供打开验证，没有中断原有窗口；构建时临时指定的二进制目标已从Cargo配置中恢复。原生结果位于 `.local/layout-rollback-review/native-tab-preview-smoke.json`。

## 悬停预览动效与现场渲染（2026-11-08）

按用户要求加入160ms淡入、120ms淡出和80ms减少动态效果模式。WAAPI只改变透明度，反转从当前屏幕透明度开始，相邻项即时更新；Esc、点击和浮层等即时动作直接取消。原生入场和离场分别准备当前网页背景，解码/绘制完成后确认原生圆角开孔；离场完成再恢复原生区域并接收关闭回执。过期背景、动画和关闭票据均忽略，当前网页保持可见。

没有预览缓存时，已有页面现场捕获；未加载项用独立临时WebView2在受信UI后方渲染并生成JPEG。临时控制器不加入正式标签、会话或历史，收起时释放。保存项缓存可跨状态广播保留，网址改变时清除旧来源；现场加载或截图失败保留明确占位提示。新标签页使用内置Rove缩小布局。原生页面与临时渲染共用原有站点profile。

前端43项单测、29项Rust测试、Clippy（`-D warnings`）、TypeScript/Vite及Debug构建通过。相关14项界面测试通过，覆盖入场/离场中间帧、离场反转无跳变、减少动态效果80ms、原生关闭回执、保存项缓存来源失效、命令栏和恢复后的旧版布局。

更新后的原生脚本9项检查通过：真实后台页面捕获和按秒刷新、临时站点现场加载、正式标签数及选中项不变、历史查询为空、控制器关闭与缓存再次命中、分屏组合、深色主题，以及160/120ms动画的可控中间帧和背景解码/回收。computer-use观察了透明度约0.398的真实离场中间帧，卡片与下方网页正确混合，没有白底或周围页面隐藏，Rove仍位于左上角。最终结果在 `.local/preview-motion-review/final/native-tab-preview-smoke.json`，测试进程已退出。

最终可执行文件为 `native/target/debug/rove.exe`，包含旧版基本布局、悬停动效与现场渲染。`rove-layout-restored.exe` 为上一轮构建，`rove-preview-motion.exe` 为本轮中间核对构建；两个临时二进制目标均未保留在Cargo配置中。

## 标题栏按钮边缘空隙（Issue #6，2026-11-08）

[Issue #6](https://github.com/haitang000/RoveBrowser/issues/6) 截图中的顶部、右侧空隙来自原生6px缩放边缘：UI WebView在这一圈被裁掉，但最小化/关闭按钮的悬停背景仍由DOM绘制。现在三个等宽按钮统一在原生绘制层完整填充，窗口圆角继续由根窗口裁切；原生命中测试仍优先处理缩放边缘，最大化按钮继续返回 `HTMAXBUTTON`。按钮按下后移出或失去鼠标捕获会取消操作。

`npm run build`、43项前端单测、29项Rust测试、Clippy（`-D warnings`）及4项窗口布局端到端测试通过。Rust检查使用真实HWND/GDI区域，覆盖100%、125%、150%、200%缩放、负屏幕坐标、四边四角的缩放命中、三个按钮的独立命中和捕获取消；像素断言核对最上方/最右侧被裁区域的悬停颜色及移出后清除。新增界面回归在480/960/1280px窗口验证DOM按钮等宽相邻、贴右对齐及原生几何上报。

computer-use实机核对浅色、深色关闭按钮的红色背景贴齐顶部和右侧，最小化按钮背景完整；实际点击最小化、最大化、还原和关闭均生效，Windows原生Snap布局菜单正常显示。没有把WebView的CDP截图作为原生合成的证明。多屏混合DPI和其他Windows版本未实机覆盖。

测试使用 `.local/issue-6/profile` 和19866调试端口。已有 `native/target/debug/rove.exe` 正在运行，因此从 `.local/issue-6/build` 源码快照构建独立的 `native/target/debug/rove-issue6.exe`，未中断原有窗口或修改正式Cargo目标；测试窗口已关闭。

用户随后反馈悬停闪动：首版在每次非客户区鼠标移动后强制同步重画，会与系统默认按钮绘制竞争；直接GDI绘制还会露出背景清除与图标绘制之间的中间状态。后续修复拦截系统的 `WM_NCPAINT` 和UxTheme标题栏绘制消息，激活事件保留winit焦点处理，并使用 [`WM_NCACTIVATE` 的 `lParam=-1`](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-ncactivate) 阻止默认重画。悬停只在目标变化时使旧/新按钮区域失效；通过 [`BeginBufferedPaint` / `EndBufferedPaint`](https://learn.microsoft.com/en-us/windows/win32/api/uxtheme/nf-uxtheme-beginbufferedpaint) 一次呈现背景及图标，不逐次覆盖系统绘制。UxTheme绘制消息的处理也参考了 [Chromium自定义窗口框架](https://github.com/chromium/chromium/blob/main/ui/views/win/hwnd_message_handler.cc)。

补充真实可见HWND的绘制计数检查：三个按钮各连续接收100次相同悬停消息后，绘制次数及无效区域均不增加；离开仅重画一次清除悬停，再次离开保持空闲。DPI像素检查改走真实缓冲绘制路径。29项Rust测试、Clippy（`-D warnings`）及Debug构建重新通过；前端未改动。实机核对关闭按钮内部移动、切换最大化按钮、Snap菜单、最大化及还原，未出现系统旧式按钮覆盖。此检查不是逐帧桌面录像。

后续验证使用独立的 `.local/issue-6/hover-profile` 和19876调试端口，测试窗口已关闭。最新可执行文件为 `native/target/debug/rove.exe`；之前的 `rove-issue6.exe` 不包含悬停闪动修复。

## Onboarding 移植验证（2026-11-12）

参考 Zigzag `78edb3201b8ec1383bedf4f2eb3b3832af6c15f4` 的 Swift onboarding，迁移五步页面与进度，并接入 Rust 权威状态、SQLite 和现有 Favorites。平台支持及具体映射见 [ONBOARDING_PORT.md](ONBOARDING_PORT.md)。

验证通过：TypeScript／Vite 构建、73 项前端单元测试、65 项 Rust 测试（`--test-threads=1`）、90 项界面 E2E（单 worker）及 Clippy（`-D warnings`）。Rust 默认并行运行时一项既有 HWND 命中测试发生冲突，串行全量复测通过。

独立临时用户目录中的真实 WebView2 验证了首次欢迎窗口显示、浏览命令隔离、常用网站落库、未完成步骤重启恢复、完成后不再显示，以及设置页重新打开。既有设置原生集成也以旧会话 fixture 复测通过，包括真实下载、SQLite 历史和 WebView2 缓存／站点数据清理、偏好重启恢复。其他原生集成脚本改用同一旧会话 fixture，未逐个全量重跑。

检查了浅色／深色和 480×400 小窗口截图。广告拦截后端尚未移植；默认应用入口只调用 Windows 设置，不修改实际默认关联，未操作用户系统默认浏览器。浏览器数据导入随后补齐，见下节。

## 数据导入实现与验证（2026-11-12）

已接入 Edge、Chrome、Firefox、Brave、Vivaldi、Opera 配置检测，真实书签／历史读取及密码 CSV 导入。书签保存到来源工作区，文件夹路径编码到名称；历史保留原时间并合并去重；密码使用当前 Windows 用户的 DPAPI 加密。导入结果和重试在引导第二步展示，原来源只读。源码映射与读取上限见 [ONBOARDING_PORT.md](ONBOARDING_PORT.md)。

TypeScript／Vite 构建、73 项前端单元测试、76 项串行 Rust 测试、91 项界面 E2E，以及生产目标 Clippy（`cargo clippy --manifest-path native/Cargo.toml -- -D warnings`）通过。新增 Rust 检查包含多配置和 Firefox profiles.ini、嵌套书签、URL 过滤、SQLite 已提交 WAL、损坏和锁定来源、访问时间换算、CSV 编码／引号／换行、DPAPI、重复导入、保留已有密码与较新历史、写入失败时回滚数据和内存状态。额外尝试的 `--all-targets -D warnings` 会触发既有 `favicon.rs:1020` 测试的 `field_reassign_with_default`，未修改该无关测试。

`node scripts/native-import-smoke.mjs` 在隔离临时用户目录中用合成 Edge／Chrome／Firefox 数据和 CSV 启动真实 WebView2，六组检查通过：来源检测与 UI 数据类型选择；历史原时间及损坏来源修复重试；DPAPI 密文、重复密码和非法来源 ID；导入工作区固定标签与历史搜索；重启恢复及用户点击选择账号后同时替换用户名／密码、不自动提交；new-password、跨 origin action、其他 origin 排除及关闭自动填充后立即移除入口。结果与界面截图保存在 `test-results/native-import/`。测试密码是本地合成数据，CSV 文件选择由 Debug fixture 路径替代，未手动验证系统文件选择对话框。

首次引导原生集成复测通过。导入界面 E2E 以模拟 IPC 覆盖来源多选、数据类型、忙状态时禁用导航、报告、重试与 480×400 布局；检查了桌面版结果截图及小窗口底部操作不重叠。依赖许可证清单重新生成，包含新增 CSV 依赖。最终 Debug 可执行文件为 `native/target/debug/rove.exe`。

## 默认浏览器与 beta6（2026-11-12）

设置页和首次引导接入当前用户的 HTTP／HTTPS 能力注册、系统默认状态查询和 Windows 默认应用入口。安装版与便携版使用相同的 `RoveURL` 命令；卸载仅清理仍属于本次安装的注册。新增按 profile 隔离的单实例管道，外部链接可冷启动或转交现有窗口，首次引导结束后打开排队链接。范围与源码见 [DEFAULT_BROWSER.md](DEFAULT_BROWSER.md)。

TypeScript／Vite 构建、73 项前端单元测试、79 项串行 Rust 测试、生产目标 Clippy（`-D warnings`）通过。本轮复测默认浏览器、设置和首次引导的 10 项界面 E2E，全数通过；上轮数据导入已执行完整 91 项界面 E2E，本轮没有重复执行其他用例。

`npm run test:native:default-browser` 的真实 WebView2 四组检查通过，覆盖首次引导排队、冷／热启动链接、无参激活、原有标签恢复、设置注册与状态 IPC，并核对真实 UserChoice 不变；结果在 `test-results/native-default-browser/checks.json`。`scripts/installer-smoke.ps1` 使用独立 AppId、注册名和中文空格安装目录，19 项检查通过，覆盖 beta5 → beta6 升级、实际安装 EXE 哈希、两个协议、启动命令和快捷方式、冷启动链接落库、卸载清理、用户数据保留及真实 UserChoice 不变。

computer-use 在 Windows 11 26200 上点击隔离 Release 窗口中的默认浏览器按钮，确认真实注册命令正确并启动 Windows 设置。系统窗口初始空白，通用默认应用页随后显示；本机没有完成 Rove 详情页和最终系统选择的交互核对，不将其计为已验证。未更改用户实际默认浏览器，测试后删除本次新增的候选注册并停止测试进程，用户原有 Rove 窗口继续运行。

发布使用静态 CRT 的 x64 Release 构建，生成安装器和便携 ZIP。许可证清单重新生成，包含 181 条记录和 289 份许可正文。便携打包会将复制载荷中超出 ZIP 支持年份的文件时间戳归一化，避免 Cargo 许可文件的 Unix 纪元时间导致压缩失败，不改动原文件正文或元数据。安装包未代码签名；其他 Windows 版本与物理多屏设备未实机覆盖。
