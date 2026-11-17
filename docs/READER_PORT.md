# Reader Mode 移植

参考 Zigzag 提交 [`79437afc94a1104f4442d52415054532e694f082`](https://github.com/ZigaoWang/zigzag/tree/79437afc94a1104f4442d52415054532e694f082) 的 `Zigzag/Web/Reader.swift`、`Sidebar/SidebarView.swift`、`Window/BrowserWindowController.swift` 和 `Resources/Reader`。Swift 不参与编译；提取脚本、阅读排版和偏好规则按行为迁移到 Rust/WebView2 与 Vue。

## 已实现

- 文章识别沿用 `isProbablyReaderable(document, { minScore: 10, minContentLength: 100 })`；可读页面在地址栏显示按钮，`Ctrl+Alt+R` 可手动尝试，快捷键可在设置中修改。
- Mozilla Readability 克隆文档提取正文。沿用上游的 Wikipedia 杂项移除、图片调整、标题站名清理、作者与发布日期处理，以及中日韩语言判断。可读文章空闲时提前解析并按 URL 缓存；SPA 地址改变后重新识别。
- 阅读视图使用独立 WebView2 控制器，原网页保留。退出不重新导航，原网页的 DOM、未提交表单与滚动位置得以保留。正文链接经宿主跳转到原标签，另开窗口请求创建标签；HTTP/HTTPS 之外的链接被忽略。
- 单栏排版、图片及图注、列表、引用、代码、表格和打印 CSS 参考原实现。Windows 下使用 Georgia、系统无衬线字体、Consolas 和微软雅黑等本机字体。
- Aa 提供 14–32 px 字号（默认 20，每步 2）、衬线／无衬线、自动／浅色／纸色／深色、29／34／40 em 栏宽。即时更新，不重新加载阅读文档；偏好保存在 SQLite 会话的 `readerPreferences` 字段，旧会话自动使用默认值。
- `Ctrl++`、`Ctrl+-`、`Ctrl+0` 在阅读中的当前标签调整文字；`Ctrl+F` 搜索阅读正文。`Esc`、阅读按钮和 `Ctrl+Alt+R` 可退出，刷新／前进／后退作用于原网页。
- 阅读页面复用宿主现有窗格布局、裁切、分屏拖动和缩略图路径。切换到其他标签关闭阅读视图；同一分屏组内保留。原网页发生导航时结束阅读，异步提取结果用请求编号与 URL 校验，避免覆盖另一个标签或文档。
- 加载中打开会等待页面完成；再按一次取消。等待 10 秒仍未完成会尝试提取已有正文，避免被迟迟不完成的资源阻塞。
- 阅读纸张和 Aa 控件以 220ms 淡入；首屏标题、段落、图片等资源从 6px 模糊渐变清晰，单组 280ms、组间隔 24ms，错峰最多 120ms。正文就绪即入场，不等待图片；首轮 10 秒内到达的可见图片单独去模糊。仅首屏内容参与错峰，结束后清理模糊图层。
- 点击退出、`Esc` 或 `Ctrl+Alt+R` 时，正文、背景与 Aa 控件在 160ms 内一起淡出，完成后恢复原网页；退场中再次切换会反向淡入。应用或系统启用减少动态效果时，禁用模糊与错峰，只保留 80ms 淡入淡出。切换标签、导航和关闭标签仍直接释放阅读控制器，避免遮挡新页面。

## 平台适配与边界

WebView2 使用 `Page.createIsolatedWorld` 和 `Runtime.evaluate` 对应 WebKit 的 `.defaultClient`。这是宿主对当前控制器的调用，发行版不开放调试端口；页面没有新增 IPC 或宿主对象。提取与识别只在主文档执行，脚本执行设 3 秒上限，结果 JSON 限 8 MiB，并校验当前 URL。动画由宿主执行可信脚本并计时；阅读控制器继续禁用网页 JavaScript，因此不使用文档定时器或事件回调。

Readability 负责提取，ammonia 负责清理 HTML、移除脚本／事件属性／嵌入页面／危险 URL，并将相对链接与图片地址解析到原网址。阅读控制器关闭页面 JavaScript，HTML 另设 CSP，不加载原网页样式或脚本。Windows 的 `with_html` 使用内部 `data:` 文档，只允许首次内部加载，后续导航交给宿主检查。

本次提供文字、图片和代码阅读，不迁移上游 macOS 外部应用链接确认、原生字体、滚动时自动隐藏 Aa、阅读视图专属过渡，以及 Shift 点击正文链接进入 Peek 的行为。阅读视图沿用 Rove 的普通新窗口标签行为。图片与原站共用 WebView2 配置，但需要特殊 Referer 或 SameSite 登录状态的图片仍可能被站点拒绝；不绕过登录、付费墙或跨源框架。

## 验证

`npm run test:native:reader` 使用独立临时资料目录、本地文章与真实 WebView2，报告保存在 `test-results/native-reader/native-reader-smoke.json`。覆盖文章识别、页面伪造解析器隔离、提取与 HTML 清理、相对图片／链接、阅读偏好即时更新、原生正文查找、退出恢复、空正文、加载等待、分屏、标签切换和 SQLite 重启恢复。真实键盘由 `scripts/real-keys.ps1` 发出，避免把 CDP 键盘事件误当作 WebView2 原生快捷键。

Rust 测试覆盖 HTML 清理、URL 解析、元数据转义、偏好边界与旧会话默认值；现有前端单元测试及查找／设置／站点控制界面测试用于回归。具体本轮结果见 [VALIDATION.md](VALIDATION.md)。

上游与新增依赖的许可证见 [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)。
