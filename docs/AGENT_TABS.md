# Agent 后台标签页

接口默认关闭。Agent 只能管理自己创建的后台标签页，不能切换工作区、接管用户的标签或模拟键鼠。每个 Rove 进程最多有 4 个 Agent 标签页。它们使用独立的 WebView2 私密配置，不共享用户的 Cookie、扩展或登录状态，也不进入 Rove 的标签会话、历史或下载列表。页面静音，权限请求、弹窗、下载、外部协议和认证对话框均被拒绝。浏览器退出后，Agent 标签不会恢复。

## 启用和调用

退出当前 Rove，或使用独立资料目录。从仓库根目录运行已构建的程序，然后在另一个终端用同样的端口和令牌调用客户端：

```powershell
$env:ERA_DATA_DIR = Join-Path (Get-Location) '.local\agent-profile'
$env:ROVE_AGENT_PORT = '19862'
$env:ROVE_AGENT_TOKEN = node -p "require('node:crypto').randomBytes(32).toString('hex')"
& .\native\target\debug\rove.exe
```

首次使用新资料目录时，先完成欢迎引导。客户端操作示例：

```powershell
$tab = node scripts/rove-agent.mjs open 'https://example.com/' | ConvertFrom-Json
node scripts/rove-agent.mjs list
node scripts/rove-agent.mjs navigate $tab.result.id 'https://example.org/'
node scripts/rove-agent.mjs close $tab.result.id
```

只有首次启动的 Rove 进程读取配置；启动第二个同资料目录的实例不会开启接口。停止使用时退出 Rove、清除 `ROVE_AGENT_PORT` 和 `ROVE_AGENT_TOKEN`，再重新启动。持有令牌的本地客户端共享同一个 Agent 会话；不要转发监听端口或将令牌写入仓库。

## 协议

- 只监听 `127.0.0.1`，不是 HTTP、MCP 或调试接口。每个 TCP 连接接收一行 UTF-8 JSON、回一行后关闭。请求最多 16 KiB，例如 `{"token":"<64 个十六进制字符>","command":{"op":"list"}}`。
- `open` 需要 `url`，`navigate` 需要 `id` 和 `url`，`close` 需要 `id`。只接受不含内嵌凭据的 HTTP/HTTPS 地址。`open` 和 `navigate` 返回的 `id` 表示导航请求已发出，不代表页面已加载。
- 响应为 `{"status":"ok","result":...}` 或 `{"status":"error","code":"..."}`。`list` 返回 `result.tabs`，每项包含 `id`、`load`、`url`、`title`。`load` 为 `loading`、`ready` 或 `failed`；尚未就绪时 `url` 和 `title` 为 `null`。命令超时后先 `list` 确认状态再重试，避免重复创建。

配置无效或端口占用时，接口不会启动。当前没有 DOM 读取、点击、表单填写或附着到用户标签的能力。控制器创建仍在宿主界面线程上，首次创建会有初始化开销。

验证：构建 Debug 程序后运行 `npm run test:native:agent`。测试使用临时资料目录和本地网页；仅用 Debug CDP 观察用户界面，不通过 CDP 控制 Agent 页面。
