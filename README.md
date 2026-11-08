# Rove

Windows 10/11 轻量浏览器基础实现：Rust + winit + wry/WebView2 + Vue 3/TypeScript + Pinia + SQLite。视觉移植自 Zigzag 的真实源码，基线、组件映射和参数见 [源码研究](docs/SOURCE_ANALYSIS.md)。这是可加载网页的 Windows 应用；Vite 地址仅用于独立界面预览。

## 开发运行

需要 Node.js 22.12+、Rust stable MSVC、Visual Studio Build Tools（C++ 桌面开发与 Windows SDK）、WebView2 Evergreen Runtime。当前环境已安装 Rust 1.99.1，Cargo 路径为 `%USERPROFILE%\.cargo\bin`；新终端需要将其加入 PATH。

```powershell
cd F:\code\EraBrowser
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
npm ci
npm run native:dev
```

`native:dev` 先构建前端，再将 dist 嵌入 exe 并启动，不依赖常驻 Vite 服务器。修改前端后重跑该命令。Rove 沿用旧版 EraBrowser 的用户数据目录、数据库和内部协议，以保留已有会话与登录状态。开发模式默认使用持久用户目录；临时测试可先设置 `$env:ERA_DATA_DIR='F:\code\EraBrowser\.local\smoke'`。

```powershell
npm run native:build
# 可执行文件：native\target\release\rove.exe
.\scripts\package.ps1 # 打包便携 zip，包含许可证
.\scripts\package.ps1 -Variant peek # 单独输出 Peek 版本，适用于旧便携版正在运行时
.\scripts\package.ps1 -Variant snap-thumbnails # 缩略图与原生 Snap 菜单版本
npm run dev       # 浏览器中预览 UI，明确标记为预览，不加载外部网页
npm test
npm run test:e2e  # 使用本机 Microsoft Edge headless
npm run test:native # 先 cargo build；本地固定网页 + Windows 原生宿主集成
npm run test:native:favorites # 本地 PNG 图标、8 个上限及离线重启缓存
npm run test:native:spaces # 先 cargo build；独立临时目录验证真实 WebView2 Space 切换
npm run test:native:preview # 先 cargo build；本地网页验证真实 WebView2 悬停预览与分屏
cargo test --manifest-path native/Cargo.toml
```

## 已实现

- 源码参数化侧栏、Favorites、固定/普通标签、选中/hover/focus、浅深主题、数字工作区、统一设计 tokens。
- 侧栏顶部置顶网站最多 8 个，所有工作区共用。通过标签页右键或拖拽添加、拖动排序、网站右键编辑和移除；移除独立管理弹窗及区域标题。网站图标从 WebView2 获取真实 PNG 并随 SQLite 会话缓存，卸载网页、离线及重启后继续显示；旧会话超出上限的项目保留为当前工作区固定标签，分屏组合一同迁移。
- 固定标签逻辑迁自 Zigzag 的 Swift Library/TabManager：保存项与运行中页面分离；关闭只卸载网页，固定项及起始网址保留；支持返回固定网址、分离为普通标签、编辑保存的名称/网址，普通标签/固定项/Favorites 间拖动转换保留同一网页实例。Favorites 跨工作区复用单个实例；已有分屏可整体固定并恢复配对。旧会话自动迁移，细节与平台差异见 [固定标签移植记录](docs/PINNED_TABS_PORT.md)。
- Windows 无边框标题栏、拖动、边缘原生 resize、最小化/最大化/关闭，逻辑坐标与 DPI 换算。
- 基本布局恢复为整行32px标题栏（Rove位于左上角）、下方240px侧栏和内缩圆角网页卡片；旧会话保存的无边框页面选项不再改变布局。工作区玻璃外观与标签悬停预览保留。
- Windows 11 最大化按钮悬停显示系统原生 Snap Layout 菜单；按钮点击支持最大化和还原，最小窗口宽度480px。Windows 10 保留普通最大化行为。
- 整个窗口采用 12px 原生圆角，标题栏、侧栏与浮层统一裁切；调整尺寸/DPI 时更新，最大化时恢复直角、还原后恢复圆角。
- 整个窗口带柔和投影，保留12px圆角；阴影随移动、缩放和 DPI 同步更新，不拦截鼠标，最小化或最大化时隐藏，还原后恢复。
- 一个独立受信 UI WebView；每个已激活网页独立 WebView2，首次激活延迟创建，切换后隐藏，关闭时销毁。
- 真网页导航、后退/前进/刷新、标题/favicon/加载状态、target=_blank 在新标签打开。
- 命令面板：在当前网页区域上方浮窗展开，保留网页背景；支持地址/Google 搜索、打开的标签页与本地历史、上下键/Enter/Esc、点击外侧关闭及焦点返回。
- 多工作区新增/重命名/颜色、独立 pins/tabs、右键跨空间移动、底栏拖动移动、标签排序。
- 侧栏触控板左右轻扫切换 Space：相邻列表跟手滑动、方向锁定、短促轻扫、首尾阻尼回弹及可中断弹簧收尾，保留各空间的列表滚动位置；横向滚轮一次只切换一个空间。点击和快捷键沿空间方向切换，背景从当前屏幕颜色平滑过渡，遵循“减少动态效果”。行为及参数移植自 Zigzag 的 `SpaceSwipe`、`transitionList` 与 `ColorView.setColor`。
- 中键关闭普通/固定标签和全部标签页视图中的卡片，关闭后台标签保留当前页；侧栏标签打开时滑入淡入、关闭时淡出、排序及相邻标签补位时平滑移动，遵循系统“减少动态效果”设置。
- 两网页独立实例分屏、侧栏拖到网页开启分屏、拖动/键盘调整比例、退出、固定组合随会话恢复。
- 原生网页圆角：对实际 wry 子窗口应用 Win32 区域裁切，按 CSS 内圆角和 DPI 换算；单页四角、分屏外角、调整尺寸和最大化时自动更新。
- 界面英文字体使用内嵌 Inter Variable（拉丁与扩展拉丁字形），中文回退 Microsoft YaHei UI；无需联网获取字体。
- SQLite WAL 保存标签/工作区/主题/关闭栈/分屏关系和历史；WebView2 自行保存站点状态。
- 全部标签页按工作区分组、过滤、点击切换和拖动移动，显示 WebView2 实际网页缩略图，分屏组合并排展示两张快照。截图在页面加载后、切换前和打开视图时更新，导航或关闭后清理；尚未加载的标签显示占位，不为截图自动打开页面。
- 标签悬停预览：普通标签、固定项及顶部常用网站停留600ms后显示320px宽的网页缩略图、标题和网址；已展开时移至相邻项立即更新，分屏组合并排展示两页。当前页及其分屏伙伴不显示预览；无缓存时在后台现场渲染，尚未正式打开的页面用临时WebView2生成画面，不增加标签或历史，收起后释放控制器。已有页面的快照每秒刷新，移出、按下、滚动、Esc、切换工作区、窗口失焦或打开浮层时收起。
- 悬停预览入场160ms淡入，移开后120ms淡出；离场中移回可从当前透明度反转，相邻项直接更新。减少动态效果时采用80ms淡入淡出；按键、点击和打开其他浮层时即时取消。原生版先解码卡片下方的网页背景再露出圆角区域，关闭先完成淡出并恢复原生网页区域，周围网页持续实时显示和交互。
- Peek：Shift 点击 HTTP/HTTPS 链接（含 target=_blank）打开独立临时 WebView2；支持 Esc、关闭按钮、点击遮罩关闭，或不重新加载地转为普通标签页。预览内 Shift 点击替换当前 Peek，临时页面不写入会话/历史；晋升后才作为普通访问记录。标签右键菜单也提供预览入口。
- Peek 动效：从链接点击位置渐入放大（240ms），关闭时向同一位置缩小淡出（180ms），中途关闭可连续反向，不跳回起点。原生网页在动画期间使用内存快照，完成后切回可交互控制器；开启“减少动态效果”时仅做80ms淡入淡出。菜单入口没有链接坐标时使用居中的轻微缩放。

快捷键：Ctrl+L/T/W/Tab/Shift+T、Ctrl+B、Ctrl+R、Alt+左右、Ctrl+Shift+反斜线。网页获得焦点时通过原生 AcceleratorKeyPressed 转发，普通网页不能调用内部命令。UI 焦点下额外支持 Ctrl+1…9 切换工作区。

## 架构与边界

`native/src/model.rs` 是标签/空间权威状态；`host.rs` 管理窗口和 WebView；`storage.rs` 保存会话和历史。Pinia 原生模式只展示 Rust 广播并发送受限命令；`preview.ts` 仅供独立前端预览。DOM 测量网页槽位，Rust 校验逻辑矩形并裁到窗口，再设置 native child bounds。没有 iframe、Electron 或完整 Chromium 源码。

原生集成脚本自动建立临时用户目录和本地 HTTP fixture，debug 构建可通过 `ERA_DEBUG_PORT` 临时开启 loopback CDP（UI=该端口、网页=下一端口）；Release 不编译该入口。原生按键路径通过 Windows 实机输入单独核验，因为 CDP 键盘事件不产生 Win32 accelerator 事件。测试结束关闭测试进程，保留临时数据库便于检查。

仅 UI WebView 注册 IPC，限定 `https://era.ui/index.html`，消息最大16KB并反序列化为封闭枚举；页面 WebView 无 IPC handler/host objects。网页侧的 `peek-links.js` 只把受信 Shift 点击转换为标准 named-window 请求，Rust 在 WebView2 NewWindowRequested 中验证 URL、用户手势及来源实例，再选择 Peek 打开方式，不提供普通网页调用内部命令的能力。UI 自定义协议只提供嵌入资源，带 CSP、禁 iframe 和外部脚本；UI 导航锁定。网页只允许 http/https（运行时空白初始化允许 about:blank）。不实现 Cookie 加密，交给 WebView2 用户目录。会话损坏时停止启动、保留数据库，错误记录到 `%TEMP%\rove-error.log`。

## 尚未完成与平台适配

- 文件夹尚未移植。Windows DOM 滚轮没有 macOS 的手势开始/结束与惯性阶段信号，Space 手势使用120ms无输入判定松手、250ms间隔区分滚动轮次；不同触控板驱动的实际手感仍需更多硬件覆盖。
- 悬停预览复用内存缩略图缓存；原生卡片保留圆角和细边线，不绘制跨越实时网页的外阴影。窄窗口会缩小卡片，分屏快照等宽展示；后台网页快照表示控制器最近可捕获的画面，不保证视频持续播放。
- 网页缩略图保留在内存，最多64张且总计8MiB；按比例缩至480×320以内、JPEG质量75，不写入会话、历史或磁盘。页面加载失败或截图失败时显示占位；已有快照代表最近捕获的画面。
- Peek 和命令栏在内存中捕获当前网页/两侧分屏快照，单张上限8MiB；等待全部背景解码并完成绘制后才抬高 UI 层。原网页在后方持续渲染，关闭时先恢复原生层级再移除快照，避免隐藏/恢复网页时闪白；命令栏展开期间显示打开时的静态背景，也支持覆盖 Peek。背景捕获或解码失败时取消本次浮层并保留原网页。设置、全部标签页等其他浮层仍临时隐藏网页，尚无背景快照。网页圆角通过原生 HWND 区域实现，GDI 使用物理像素区域栅格化。
- 标题栏使用 WebView2 原生拖动区域，四边与四角由 Win32 命中测试直接处理缩放；Snap Layout 菜单由 Windows 提供，是否显示遵循系统版本和设置。多屏幕混合 DPI、各类 Snap 区域及最大化边界仍需更多实机覆盖。
- 侧栏折叠采用源码180ms时长，尚未复制原生浮动侧栏的边缘呼出。分屏比例暂不跨重启保存（组合关系保存）。
- 图标为自绘线图标，SF Pro 换为 Inter 和 Windows 中文字体；frame 已按原始 OKLab 公式生成，accent/ink 尚未移植完整 Brand.Theme 求解；没有宣称像素级一致。
- 尚无安装器/自动更新/扩展支持/站点权限管理界面。不是生产发行版。

验证结果详见 [验证记录](docs/VALIDATION.md)。第三方来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
