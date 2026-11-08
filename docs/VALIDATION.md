# 实际验证记录

日期：2026-11-07。Windows 本机，MSVC Build Tools 18，Rust 1.99.1，Node 24.18.0，WebView2 Evergreen 156/157 已安装。没有使用 Electron、完整 Chromium 源码或 iframe。

| 检查 | 实际结果 |
|---|---|
| 上游检出 | `git clone --depth 1` 成功；提交 `5338e99f58aa9a75a0696d5d96ec74353c3b3c7d` |
| Vue/TypeScript | `npm run build` 成功，vue-tsc 严格类型检查 + Vite 生产构建 |
| 前端单元测试 | `npm test`：12/12，覆盖 URL、预览状态、分屏引用、圆角、Peek布局、链接缩放几何及常用网站校验/编辑/排序 |
| 界面端到端 | `npm run test:e2e -- --workers=1`：22/22；含布局/快捷键/空间/主题/分屏、480px最小宽度、Peek 动画、标签中键/排序/减少动态效果及顶部常用网站自定义 |
| Rust 单元/存储测试 | `cargo test --manifest-path native/Cargo.toml`：17/17，含真实 HWND/DPI 命中区域、GDI裁切、缩略图压缩与缓存边界、状态/SQLite及常用网站保存/清空后重新加载 |
| Rust 静态检查 | `cargo clippy --manifest-path native/Cargo.toml -- -D warnings` 通过 |
| Debug 构建 | `cargo build --manifest-path native/Cargo.toml` 成功 |
| Release 构建 | `cargo build --release --manifest-path native/Cargo.toml` 成功；exe 约4.14 MiB（含本地 Inter 字体和图片编解码，不含系统 WebView2 Runtime） |
| 原生集成 | `npm run test:native`：29项通过；本地固定页面 + 实际 Windows 宿主/WebView2，含真实缩略图像素、整体圆角、中键关闭、侧栏过渡、Peek 动画、常用网站自定义和会话恢复 |
| Windows 实机 | 使用 computer-use/sky 打开 Debug 和 Release；加载外部 example.com 与 github.com，观察实际网页文档和标题同步，网页与240宽侧栏边界分离；从网页焦点触发 Ctrl+L 打开受信 UI；Release 最大化后布局正确 |
| 来源/许可 | 保存上游 LICENSE/NOTICE/商标说明；提取162项锁定依赖记录（含 Rust 构建依赖）及256份可用许可/notice文本，包含 Inter 的 OFL 和 image 编解码依赖 |

原生集成8项：Inter 字体实际加载；真实页面加载/标题；网页 `window.ipc` 不存在且伪造 WebMessage 不执行内部命令；前进/后退；UI Ctrl+L/浮层恢复；双独立 WebView 分屏；正常退出并重启后恢复标签、固定和分屏关系；SQLite 历史检索。使用临时 profile，不触碰用户已有浏览数据。执行后 `test-results/native-smoke.json` 保存结果和临时目录。

## 顶部常用网站自定义

通过侧栏“常用网站”旁的设置按钮、常用网站右键或“工作区与外观”进入管理。支持添加、编辑名称和网址、上移/下移、移除，所有工作区共用，最多32项；标签页右键可直接添加。地址自动补全 HTTPS，前端和原生端均拒绝无效协议、带登录信息的网址和规范化后重复的网址。

新增4项界面端到端验证，覆盖上述编辑流程、跨工作区打开和复用已有标签、重复/无效输入、480×400窗口及清空后重新添加。删除当前聚焦条目后，Esc仍可关闭设置并恢复入口焦点；右键菜单打开时会获得键盘焦点。

真实Windows宿主集成使用独立临时用户目录和本地HTTP页面，实际操作管理表单后经IPC写入SQLite，关闭并重新启动进程，确认自定义名称、网址、顺序和移除结果恢复；点击常用网站时已有网页的DOM标记保留，没有重新加载。Rust存储测试另行确认清空列表后重开数据库仍为空，不会恢复默认网站。

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

尚未完成标签悬停预览、触控板惯性空间切换、浮动侧栏边缘呼出。分屏比例不跨重启保存。Peek 已提供临时页面及背景快照，其他浮层仍隐藏网页而不保留页面快照背景。

## 真实缩略图与原生 Snap 菜单

Desk 卡片使用 WebView2 CapturePreview 获取实际页面，再在工作线程按比例缩至480×320以内，编码为质量75的 JPEG。原生集成检查图片可解码、尺寸受限，且实际像素与蓝色本地 fixture 相符；不是仅检查图片标签或 data URL。结果和截图保留在 `.local/snap-native-results/native-smoke.json`、`native-thumbnails.png`，27项完整检查通过。

缩略图仅存在内存，最多64张、8MiB；导航版本变化后拒绝旧异步结果，关闭标签后清理缓存，不写入SQLite或用户历史。尚未创建的网页不会为了截图自动加载，失败时显示占位。Peek 的全尺寸捕获与动画优先，避免后台缩略图占用同一个渲染器的捕获流程。

最大化按钮通过根窗口 WM_NCHITTEST 返回 HTMAXBUTTON，由 Windows 11 显示原生 Snap Layout 菜单。真实 HWND 测试覆盖负屏幕坐标、1×/1.5×/2× DPI、UI区域挖孔，以及不抢占相邻最小化/关闭按钮。computer-use 实机观察到了系统提供的六种布局悬停菜单；实际点击最大化到屏幕工作区后，再点击可还原，按钮首帧及主题颜色正常。最小窗口宽度改为480px，并通过界面尺寸检查。

菜单是否启用由系统设置决定；Windows 10 保留普通最大化。尚未实机覆盖系统菜单中每个布局区域的选择、多屏混合 DPI 和所有 Windows 版本。参考：[Microsoft 自定义标题栏接入 Snap Layout](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-snap-layout-menu)。

## 原生圆角与字体后续验证

`native/src/clip.rs` 使用 [SetWindowRgn](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowrgn) 限制实际网页窗口的绘制区域。区域成功安装后交给 Windows 管理，创建失败或未安装的临时 GDI 对象由 RAII 清理；仅尺寸/圆角变化时重建。未向普通网页添加任何新 IPC 或脚本。

使用两个高对比纯色的本地真实网页检查了单页四角、双实例分屏底部外角、顶部标题条直角接缝，以及最大化后的边界。字体从自定义协议提供 `font/woff2` 资源，原生集成测试确认 FontFace 状态为 loaded；无需外部字体服务。

尚未覆盖多屏混合 DPI、全部 Windows 10/11 版本、系统 Snap 的完整行为、音视频会议、复杂登录/弹窗及大规模长时间使用。窗口当前使用 winit 真实 drag/drag_resize/maximize；上述未覆盖项不作为通过声明。所有剩余平台差异和功能范围也在 README 中列出。

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
