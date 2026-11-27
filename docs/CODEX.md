# 用 Codex 操作 Rove 标签页

在 Rove 的浏览器菜单中选择「复制 Codex MCP 配置」。将配置添加到当前 Codex 项目的 `.codex/config.toml`，或添加到用户配置 `~/.codex/config.toml`。配置中的命令指向当前 Rove 程序和资料目录。重新打开 Codex 后，`rove` 会作为本地 MCP 服务出现。 [Codex MCP 配置](https://learn.chatgpt.com/docs/extend/mcp?surface=cli#configure-with-configtoml)

在 Rove 打开所需网页，等页面加载完毕，再从浏览器菜单选择「允许 Codex 使用此标签页」。在 Codex 中指定要读取或操作的内容。接入提供网页快照、点击、填写和滚动，作用于共享页的主文档。

地址栏右侧的 `Codex` 按钮表示页面正在共享，点击即可停止。切换或关闭标签、导航到其他来源或退出 Rove 也会结束共享。共享授权只在当前浏览器进程中有效。

MCP 服务通过标准输入输出运行。Rove 在首次共享时创建经过身份验证的本机连接，连接凭据由当前 Windows 用户账户加密。 [原生接口测试](../scripts/native-codex-mcp-smoke.mjs)
