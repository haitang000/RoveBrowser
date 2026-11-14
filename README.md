# Rove

Rove 是一款面向 Windows 10/11 的实验性浏览器，使用 WebView2 加载网页，以工作区、侧栏标签和分屏组织浏览。界面与部分交互参考并移植自 Zigzag；项目仍处于 beta 阶段，不是生产发行版。

[下载发行版](https://github.com/haitang000/RoveBrowser/releases) · [反馈问题](https://github.com/haitang000/RoveBrowser/issues) · [验证记录](docs/VALIDATION.md)

## 功能概览

- **工作区和标签**：按工作区组织标签、固定标签和分屏；顶部常用网站跨工作区共享，最多 8 个。支持拖动排序、移动标签和悬停预览。拖动侧栏右边缘可调整宽度，重启后保留；双击边缘恢复默认宽度。
- **浏览与分屏**：导航、历史、下载、页面查找，以及两个独立网页并排浏览。Shift 点击链接可使用 Peek 临时预览，再将其转为普通标签。
- **命令面板和设置**：通过命令面板搜索地址、标签及历史；设置页提供搜索引擎、外观、下载目录、快捷键和浏览数据管理等选项。
- **首次引导和导入**：可从 Edge、Chrome、Firefox、Brave、Vivaldi、Opera 的配置导入书签和历史，并从 CSV 导入密码。导入的密码使用 Windows DPAPI 加密，并需用户手动选择填充。
- **会话恢复**：工作区、标签和历史保存在 SQLite；网站登录等站点状态由 WebView2 管理。支持通过 Windows 设置将 Rove 设为 HTTP/HTTPS 链接的默认浏览器。

默认快捷键包括 `Ctrl+L`、`Ctrl+T`、`Ctrl+W`、`Ctrl+Shift+T`、`Ctrl+Tab`、`Ctrl+B`、`Ctrl+R`、`Alt+←/→` 和 `Ctrl+,`；`Ctrl+1` 至 `Ctrl+9` 切换工作区。快捷键可在设置中修改。

## 安装与使用

从 [GitHub Releases](https://github.com/haitang000/RoveBrowser/releases) 下载 Windows x64 安装程序（`*-setup.exe`）或便携 ZIP。需要 Windows 10/11 x64，或能够运行 x64 应用的 Windows ARM64，以及 Microsoft Edge WebView2 Evergreen Runtime。

- **安装版**：运行安装程序，默认安装到 `%LOCALAPPDATA%\Programs\Rove`，无需管理员权限。已有 WebView2 Runtime 时可离线安装；缺少时安装程序会联网调用微软引导程序。安装包目前未签名。
- **便携版**：解压 ZIP，运行 `Rove.exe`。需要预先安装 WebView2 Runtime。运行发行版无需 Node.js 或 Rust。

卸载或覆盖升级会保留用户数据。Rove 沿用旧版 EraBrowser 的用户数据目录、数据库和内部协议，以保留已有会话与登录状态；便携版默认也使用同一目录，并非将资料保存在程序文件夹中。测试时可通过 `ERA_DATA_DIR` 指定独立目录。

在「设置 → 通用 → 设为默认浏览器」或首次引导最后一步打开 Windows 默认应用设置，再由用户选择 Rove。便携版移动位置后，可再次点击该按钮更新注册路径。当前仅注册 HTTP/HTTPS 链接，不关联本地 HTML 或 PDF；详情见 [默认浏览器支持](docs/DEFAULT_BROWSER.md)。

## 开发

### 环境要求

- Windows 10/11、Microsoft Edge WebView2 Evergreen Runtime
- Node.js 22.12 或更新版本（发行工作流使用 Node.js 22）
- Rust stable 的 Windows MSVC 工具链、Visual Studio Build Tools 的 C++ 桌面开发组件和 Windows SDK
- 构建安装程序另需 [Inno Setup 6](https://jrsoftware.org/isinfo.php)

在仓库根目录的 PowerShell 中运行：

```powershell
npm ci
npm run native:dev
```

`native:dev` 先构建前端，再将 `dist` 嵌入原生程序并启动；修改前端后需重新运行。为避免使用日常资料，可在启动前设置独立目录，例如：

```powershell
$env:ERA_DATA_DIR = Join-Path (Get-Location) '.local\dev-profile'
npm run native:dev
# 恢复默认目录：Remove-Item Env:ERA_DATA_DIR
```

仅预览界面时可运行 `npm run dev`，并打开终端显示的地址。该模式不是真正的浏览器宿主，不能用于验证外部网页和 Windows 原生功能。

### 构建与打包

```powershell
npm run native:build  # 前端 + 原生 Release；输出 native\target\release\rove.exe
.\scripts\package.ps1 # 将上述构建打成 release 下的便携 ZIP
npm run installer     # 构建 x64 Release 和安装程序，输出到 release
```

安装器脚本会下载并校验微软 WebView2 引导程序。若 Inno Setup 编译器不在默认位置，可设置 `$env:INNO_SETUP_COMPILER` 为 `ISCC.exe` 的路径。已有同一源码的安装器构建产物时，可用 `npm run installer -- -SkipBuild` 重新打包。发行工作流同时生成安装 EXE 和便携 ZIP。

### 测试

```powershell
npm run build                           # TypeScript 检查和前端构建
npm test                                # 前端单元测试
npm run test:e2e                        # 界面测试，使用本机 Microsoft Edge
cargo test --manifest-path native/Cargo.toml -- --test-threads=1
cargo build --manifest-path native/Cargo.toml
npm run test:native                     # Windows 原生宿主 / WebView2 冒烟测试
npm run test:native:sidebar             # 侧栏调宽、真实视口和重启恢复专项（需先构建 Debug）
```

原生冒烟测试需要先构建 Debug 程序；其他专项测试及其构建前提见 `package.json` 的 `test:native:*` 脚本。原生测试使用临时用户目录和本地测试网页；实际验证范围和已知测试限制见 [验证记录](docs/VALIDATION.md)。

## 架构

- `src/`：Vue 3、TypeScript、Pinia 界面；独立预览使用 `src/lib/preview.ts`。
- `native/`：Rust、winit 和 wry/WebView2 宿主；`model.rs` 管理标签与工作区状态，`host.rs` 管理窗口和 WebView，`storage.rs` 保存会话及历史。
- `installer/`、`scripts/`：安装器、打包及测试脚本。
- `docs/`：源码分析、功能移植说明与验证记录。

受信的应用界面通过受限 IPC 与原生宿主通信；普通网页使用独立 WebView，不提供内部命令接口。应用不使用 Electron 或 iframe 作为网页浏览器。

## 已知限制

- 广告拦截后端、文件夹、自动更新、扩展和站点权限管理界面尚未实现。
- 分屏组合可恢复，但分屏比例暂不跨重启保存；侧栏尚无原生浮动侧栏的边缘呼出。
- Windows 触控板手势、多屏混合 DPI 和部分窗口布局仍需更多硬件验证；网页缩略图可能显示最近捕获的画面或占位。
- 视觉和交互参考 Zigzag，但 Windows 字体、窗口及平台能力存在差异，不保证像素级一致。

更多细节见 [源码研究](docs/SOURCE_ANALYSIS.md)、[移植候选](docs/PORT_CANDIDATES.md)、[固定标签](docs/PINNED_TABS_PORT.md)、[首次引导](docs/ONBOARDING_PORT.md)、[设置](docs/SETTINGS_PORT.md) 和 [验证记录](docs/VALIDATION.md)。

## 许可证与致谢

本项目采用 [Apache License 2.0](LICENSE)。Zigzag 及其他第三方组件的来源、许可和声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 与 [licenses/](licenses/)。
