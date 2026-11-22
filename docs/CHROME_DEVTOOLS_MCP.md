# Chrome DevTools MCP

Rove 可以让官方 Chrome DevTools MCP 连接正在运行的 WebView2 网页。接口默认关闭。启用后，WebView2 为网页环境分配本机回环端口，MCP 服务通过该环境的 `DevToolsActivePort` 文件发现端口。Rove 界面和独立的 Agent 后台标签使用各自的 WebView2 环境。

## 启用接口

在「设置 → 实验性」打开 **Chrome DevTools MCP**，退出并重新启动 Rove，再打开一个网页。WebView2 会在第一个网页环境启动时写入端口文件。使用完毕后在同一栏关闭开关，并重启 Rove。

调试端口无身份验证，启用期间本机进程可以访问该资料中的网页与站点会话。需要独立资料时，可以从仓库根目录运行以下 PowerShell 命令，再在该实例的设置中启用开关。

```powershell
$env:ERA_DATA_DIR = Join-Path (Get-Location) '.local\devtools-profile'
& .\native\target\debug\rove.exe 'https://example.com/'
```

脚本仍可在启动时设置 `ROVE_DEVTOOLS_MCP=1` 临时启用，设置中会显示开关由启动变量控制。清除变量并重启后，Rove 会使用已保存的开关值。

## 配置 MCP 客户端

MCP 服务需要 Node.js 22.12 或更新版本。在支持 `mcpServers` 格式的客户端中添加以下配置，将 `--user-data-dir` 后的值替换为实际的 WebView2 资料路径。默认路径为 `%LOCALAPPDATA%\Era\EraBrowser\data\WebView2\EBWebView`。如果启动时指定了 `ERA_DATA_DIR`，则使用该目录下的 `WebView2\EBWebView`。请在配置中填入展开后的绝对路径。VS Code 使用 `servers` 作为顶层键。

```json
{
  "mcpServers": {
    "rove-devtools": {
      "command": "npx",
      "args": [
        "-y",
        "chrome-devtools-mcp@1.13.0",
        "--auto-connect",
        "--user-data-dir",
        "C:\\src\\RoveBrowser\\.local\\devtools-profile\\WebView2\\EBWebView",
        "--no-usage-statistics",
        "--no-performance-crux"
      ]
    }
  }
}
```

连接后先调用 `list_pages`，再把返回的页面 ID 传给 `take_snapshot`、`take_screenshot` 等网页工具。`new_page` 可以创建临时调试目标，再用 `close_page` 关闭。需要将网页加入 Rove 工作区和会话时，请在 Rove 中打开标签。

如需验证接入，从仓库根目录运行 `npm run test:native:devtools-mcp`。脚本使用临时资料和本地网页，通过 stdio 调用官方 MCP 服务。默认测试 Debug 程序，可用 `ERA_SMOKE_EXE` 指定已构建的 Release 程序。

参考 [Microsoft WebView2 接入说明](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/devtools-mcp-server#auto-connect-to-a-webview2-instance) 和 [Chrome DevTools MCP 配置](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/configuration.md)。
