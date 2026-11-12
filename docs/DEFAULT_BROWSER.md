# Windows 默认浏览器

从「设置 → 通用 → 设为默认浏览器」或首次引导最后一步进入 Windows 默认应用设置，再选择 Rove。Windows 11 使用系统 URI 请求定位到 Rove 页面；如果系统只显示默认应用列表，可搜索 Rove，旧系统可选择网页浏览器。应用不会写入 UserChoice／Hash 或强制抢占默认项。设置页分别检测 HTTP 和 HTTPS，部分关联和全部完成使用不同提示；窗口重新获得焦点时刷新。首次引导中已经默认时不重复打开系统设置。

## 注册与安装

安装器在 HKCU 注册 `Software\Classes\RoveURL`、`Software\Clients\StartMenuInternet\Rove\Capabilities` 和 `Software\RegisteredApplications\Rove`，不需要管理员权限。Capabilities 包含 HTTP／HTTPS，图标与启动命令指向实际安装位置；启动命令为 `"<路径>\Rove.exe" -- "%1"`，支持中文和空格路径。当前只声明网页链接，不关联应用尚未支持的本地 HTML、PDF 或其他协议。

便携版点击设置按钮时使用相同结构注册当前可执行文件；移动程序后再次点击即可更新位置。安装升级更新可执行路径和版本。卸载先检查命令是否仍指向本次安装，再删除相应 ProgID、Capabilities 和 RegisteredApplications 值；若另一份便携程序后来接管注册，则保留它。用户数据和系统 UserChoice 保留，卸载后的默认应用由 Windows 处理。

`native/src/default_browser.rs` 实现候选注册、`AssocQueryStringW` 查询和设置页面启动；`installer/Rove.iss` 使用同一协议、ProgID 和路径格式。Rove 的网页渲染器不具备调用这些操作的 IPC，只有可信应用界面可发出设置命令。

## 外部链接和进程

`native/src/instance.rs` 在打开 SQLite／WebView2 前，为规范化的用户数据路径取得一个本地 Windows named pipe 的首个实例。第二次启动把经过验证的 URL 交给现有进程并退出；不带参数时唤回窗口。不同资料目录独立运行，同一资料目录避免多个进程同时保存会话。管道拒绝远程客户端，消息最多 64 KiB，单次最多 16 个 HTTP(S) URL；不接受任意 Chromium 开关、内部协议、脚本、本地文件或 URL 内的用户名／密码。

新链接以普通标签页打开，保留原有标签；首次引导期间最多保留 32 个待处理链接，完成后再打开。关闭和重启沿用原会话持久化。默认浏览器选择本身由系统完成，不代表 Rove 可以处理应用未支持的协议。

## 验证

Rust 测试在独立 HKCU 子树验证中文空格路径、幂等注册和保留 UserChoice；本地 named pipe 测试覆盖链接验证、Unicode、消息大小和重复进程转交。`npm run test:native:default-browser` 使用真实 WebView2、隔离用户目录与仅 Debug 可用的 `ERA_ASSOCIATIONS_FIXTURES=Software\Rove\Tests\<唯一名称>`，覆盖首次引导排队、冷／热启动、快捷方式激活、原有标签恢复和设置 IPC；测试不更改真实默认项。

`scripts/installer-smoke.ps1 -CompilerPath <ISCC.exe>` 使用正式发布载荷编译独立 AppId 和注册名称的安装器，验证 beta5 升级、注册命令、HTTP／HTTPS 能力、中文安装路径启动、卸载清理与资料保留。正常生产编译使用固定 Rove 标识，测试不覆盖已有 Rove 安装。真实 Windows 设置页面的可见性单独核对，测试不选择新的默认浏览器。
