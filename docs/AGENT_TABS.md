# Agent 标签页

接口默认关闭。启用桥接后，Agent 可以管理自己创建的后台标签页。每个 Rove 进程最多有 4 个后台标签页。它们使用独立的 WebView2 私密配置，不共享用户的 Cookie、扩展或登录状态，也不进入 Rove 的标签会话、历史或下载列表。页面静音，权限请求、弹窗、下载、外部协议和认证对话框均被拒绝。浏览器退出后，后台标签不会恢复。

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

## 普通浏览器标签页

在启动 Rove 的终端额外设置 `$env:ROVE_AGENT_BROWSER_TABS = '1'`，再启动浏览器。持有桥接令牌的客户端即可列出和操作当前资料中各工作区的普通标签页。打开和导航会使用该资料的登录状态与浏览记录设置，标签变化按普通浏览会话保存。移除这个环境变量并重启，即可撤销普通标签页访问。

```powershell
node scripts/rove-agent.mjs browser-list
$tab = node scripts/rove-agent.mjs browser-open 'https://example.com/' | ConvertFrom-Json
node scripts/rove-agent.mjs browser-navigate $tab.result.id 'https://example.org/'
node scripts/rove-agent.mjs browser-activate $tab.result.id
node scripts/rove-agent.mjs browser-close $tab.result.id
```

## 网页内操作

启用普通标签页访问后，在启动 Rove 的终端再设置 `$env:ROVE_AGENT_PAGE_ACTIONS = '1'`。网页命令作用于当前显示的 HTTP 或 HTTPS 网页。用 `browser-list` 找到目标标签的 ID，再切到该标签并读取快照。

```powershell
$id = '<标签页 ID>'
node scripts/rove-agent.mjs browser-activate $id
$page = (node scripts/rove-agent.mjs page-snapshot $id | ConvertFrom-Json).result
$page.text
$page.elements | Format-Table element, kind, label
# 按实际返回的编号，将下方的 2 和 3 替换成目标文本框和按钮。
node scripts/rove-agent.mjs page-fill $id $page.snapshot 2 '搜索内容'
node scripts/rove-agent.mjs page-click $id $page.snapshot 3
```

快照包含网页标题、地址和最多 12,000 个 UTF-16 码元的正文。每页返回最多 100 个可操作元素，通过 `page-snapshot <id> <nextOffset>` 继续列出。再次读取、翻页或网页导航后，使用新快照和编号。目标被移除、禁用或改名后，重新读取快照。

`page-fill` 替换普通文本框或多行输入框的内容，最多 4096 字节。快照不会读取输入框中的现有值，密码框、文件选择框和 iframe 不在操作范围内。点击和填写通过网页的 DOM 事件执行，返回 `applied: true` 表示事件已发出，可以再次读取快照确认结果。

## 协议

- 只监听 `127.0.0.1`，不是 HTTP、MCP 或调试接口。每个 TCP 连接接收一行 UTF-8 JSON、回一行后关闭。请求最多 16 KiB，例如 `{"token":"<64 个十六进制字符>","command":{"op":"list"}}`。
- `open` 需要 `url`，`navigate` 需要 `id` 和 `url`，`close` 需要 `id`。只接受不含内嵌凭据的 HTTP/HTTPS 地址。`open` 和 `navigate` 返回的 `id` 表示导航请求已发出，不代表页面已加载。
- 响应为 `{"status":"ok","result":...}` 或 `{"status":"error","code":"..."}`。`list` 返回 `result.tabs`，每项包含 `id`、`load`、`url`、`title`。`load` 为 `loading`、`ready` 或 `failed`。尚未就绪时 `url` 和 `title` 为 `null`。
- 普通标签页命令使用 `browser_list`、`browser_open`、`browser_navigate`、`browser_activate` 和 `browser_close`。未启用普通标签页访问时返回 `browser_tabs_disabled`。命令只接受对应标签列表中的 ID。
- `browser_list` 接受可选的非负整数 `offset`，默认从第一个标签开始。每页最多 8 项，包含 `id`、`spaceId`、`title`、`url` 和 `loading`。地址超出 8192 字节时返回 `null`。结果还包含 `activeTab` 和 `activeSpace`。继续分页时传入 `nextOffset`，值为 `null` 表示已到末页。
- `browser_open` 需要 `url`，在当前工作区打开并选中新标签。`browser_navigate` 需要 `id` 和 `url`。两者都返回标签的 `id`，只接受不含内嵌凭据的 HTTP/HTTPS 地址。页面加载是异步的。对于尚未创建页面的后台标签，`browser_navigate` 返回 `deferred: true`，等标签切到前台再加载。如果标签正在进入休眠，返回 `tab_busy`，稍后可重试。
- `browser_activate` 和 `browser_close` 需要 `id`，并返回操作后的 `activeTab`。如果操作超时，先用对应的列表命令确认状态再重试，避免重复创建。
- 网页命令使用 `page_snapshot`、`page_click` 和 `page_fill`。未开启网页操作时返回 `page_actions_disabled`。三种命令都需要 `id`。`page_snapshot` 接受可选的 `offset`，返回 `url`、`title`、`text`、`textTruncated`、`snapshot`、`elements` 和 `nextOffset`。`page_click` 和 `page_fill` 需要快照令牌 `snapshot` 和整数编号 `element`，填写还需要 `text`。
- 页面未加载完时返回 `page_not_ready`，切到其他标签时返回 `tab_not_visible`。遇到 `page_changed`、`stale_snapshot` 或 `stale_element` 时，读取新的快照后再选择元素。

配置无效或端口占用时，接口不会启动。控制器创建仍在宿主界面线程上，首次创建会有初始化开销。

验证：构建 Debug 程序后运行 `npm run test:native:agent`。测试使用临时资料目录和本地网页；仅用 Debug CDP 观察用户界面，不通过 CDP 控制 Agent 页面。
