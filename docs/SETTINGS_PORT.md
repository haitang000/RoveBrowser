# Zigzag 设置移植

研究源码：<https://github.com/ZigaoWang/zigzag/tree/12183cbaf5acf68ac4a23a1d5003b1b10d587e03>。2026-11-08 获取 main 并实际阅读 `Settings/SettingsView.swift`、`SettingsPage.swift`、`Shortcuts.swift`、`App/Library.swift` 的 `SearchEngine`、`Design/Tokens.swift` 的 `Page`，以及 `FavoritesView.swift` 的列数设置。原始 AppKit/Swift 实现转写为 Vue/TypeScript、Rust 和 WebView2，未编译 Swift。

## 页面和交互

- 侧栏底部齿轮、命令面板的 Settings、Ctrl+, 均复用当前窗口已有设置标签页，优先选择当前工作区中的设置，再选择其他工作区中的设置；没有才创建。
- 地址为 `rove://settings`，分类为 `/spaces`、`/passwords`、`/privacy`、`/translation`、`/extensions`、`/shortcuts`。分类切换更新该标签页地址；关闭、恢复、分屏和重启沿用普通标签页生命周期。已保存的旧 `era://settings` 地址及关闭记录自动迁移到新地址，输入旧地址同样转为新地址。设置由受信外壳绘制，真实网站仍无 IPC。
- 保留原版 7 个分类和跨分类逐词搜索，按分类／分组标题或具体设置文字匹配，隐藏的自定义搜索地址不参与搜索。没有匹配时显示空状态。
- `Tokens.Page`：分类栏 240px，最大表单内容宽 880px，外侧留白 28px，窄页面 16px，行内留白 16px，分组间距 24px。720px 以下缩为 64px 图标栏，520px 以下改为标题下方的横向导航。行在控制项与说明挤压时换行，无表单背景卡片，细分隔线从文字起点开始。
- 保留现有加号入口的工作区快速编辑弹窗；全局设置新增独立入口。

## 已接入的选项

| 设置 | 实际行为 |
| --- | --- |
| 搜索引擎 | Google、DuckDuckGo、Brave、Bing、Kagi、Ecosia、Startpage、Perplexity、Yahoo、百度、Yandex、自定义；命令栏搜索与 Rust 直接地址解析共用选择 |
| 自定义搜索地址 | 必须为 HTTP/HTTPS、有主机和 `%s`，拒绝用户名／密码；搜索词逐字节编码，支持路径或查询参数位置；空模板回退 Google |
| 搜索建议 | 默认开启；当前只对已有 Google 请求实现生效，其他引擎不向 Google 发送输入；关闭后 Rust 不再发起请求，前端清空并拒绝已过期结果 |
| 浅色／深色 | 使用现有主题命令，界面与网页 profile 主题同步 |
| 工作区大标题 | 关闭后改为普通 13px 标题 |
| 每行常用网站 | 自动、2、3、4 列，立即调整现有常用网站布局 |
| 减少动态效果 | 应用偏好与系统偏好取逻辑或；侧栏切换、背景色、Peek、预览和 CSS 动效共同遵循 |
| 退出前询问 | 默认开启，窗口关闭按钮及 Alt+F4 均经过 Win32 确认框；取消保留运行状态 |
| 下载目录 | 验证已有绝对目录，更新 WebView2 profile 并在真实 DownloadStarting 中应用；同名文件使用数字后缀；留空回到系统下载目录 |
| 工作区 | 新建、编辑名称／颜色／清透／磨砂／通透程度、拖动排序；在设置中新建不会离开当前设置页；删除需确认且至少保留一个，共用常用网站保留 |
| 密码和表单 | WebView2 的提示保存密码、自动填充表单开关，同时应用到已创建和后续创建的网页；密码存储仍由运行时负责 |
| 浏览历史 | 可停用后续本机历史写入；清除历史同时清空已关闭标签栈 |
| 缓存／网站数据 | 分别调用 WebView2 profile 数据清理，Cookie 与全部 DOM 存储一起清除；等待完成回调后显示结果；不清理受信 UI profile |
| 快捷键 | 录制、清除、恢复默认、冲突校验；12 个操作由 UI 和原生网页使用同一配置，按钮提示同步更新；Ctrl+1–9 保留给工作区，Alt+F4 保留给系统 |

所有设置保存于现有 SQLite session 的 `settings` 字段，旧会话缺少字段时填入默认值，不改用户目录或站点 profile。未知搜索引擎回退 Google；移动磁盘上的下载目录失效后回到系统默认，不重置其他偏好。

## 平台和功能边界

原版的系统语言设置、iCloud／Contacts 密码功能、Apple 翻译、WKWebExtension、Sparkle 更新、导入器、EasyList 拦截、自动归档、自动画中画、逐站点权限／存储列表均依赖当前项目尚未提供的独立子系统。本次未伪造这些功能的开关。翻译与扩展分类保留，明确显示暂未提供；密码分类提供 WebView2 能实际控制的选项，没有自建密码管理器。

默认浏览器按钮打开 Windows 默认应用设置；当前安装器和便携程序尚未提供 ProgID 注册，不能承诺在系统列表中直接设为默认。下载目录使用完整路径输入，与原版 macOS 文件夹菜单不同。保留用户要求恢复的整行标题栏和圆角网页卡片，未重新启用之前停用的无边框布局。快捷键提供当前项目已有的操作，Windows 使用 Ctrl 替代 Command。

## 验证

本轮前端构建通过，54 项前端单元测试、38 项 Rust 测试通过。前端 E2E 共 63 项：全量运行 61 项通过；更新 Peek 的标签可访问名称断言后，该项与一次页面加载超时项定向重跑通过。最后的快捷键提示改动另行重跑设置交互 4 项，全部通过。原生设置联调使用独立构建和临时用户目录通过，未覆盖正在运行的调试程序。前端 E2E 使用本地临时配置的 5181 端口，避免与 5173 上的已有服务冲突。

- `npm run build`、`npm test`：类型检查、前端构建，以及内部地址、搜索编码、模板校验、快捷键冲突和工作区生命周期。
- `cargo test --manifest-path native/Cargo.toml`：原生设置校验、设置页跨空间复用、序列化／修复、搜索引擎及删除工作区行为。
- `tests/e2e/settings.spec.ts`：独立设置页、跨分类搜索、即时外观和布局偏好、自定义引擎、实际 UI 快捷键变化、编辑／确认删除、窄窗口无横向溢出。
- `npm run test:native:settings`：隔离用户目录与 loopback 页面验证真实 WebView2 搜索、自定义下载目录／同名文件、设置与真实网页分屏及原生网页尺寸同步、历史／缓存／Cookie／localStorage 清除和重启恢复；结果在 `test-results/native-settings/`。CDP 测试先恢复默认下载行为，避免 Playwright 默认拒绝下载，不向 CDP 提供下载目标路径。可用 `ERA_TEST_EXE` 指定独立构建的可执行文件，避免覆盖正在运行的调试程序。
