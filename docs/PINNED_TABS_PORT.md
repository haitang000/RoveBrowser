# Zigzag 固定标签逻辑 → Rove

基线：Zigzag `5338e99f58aa9a75a0696d5d96ec74353c3b3c7d`。实际阅读 Swift 文件后，将状态规则重写为 Rust 原生模型和 TypeScript 界面预览；没有运行 Swift 或引入 WKWebView。

| Swift 源码规则 | Rove 实现 |
|---|---|
| `Library.Bookmark` 的稳定 ID、名称、起始网址；空间 pins 与全局 favorites | `State.pins` / `State.favorites`，通过 `Tab.bookmarkId` 关联运行中的网页 |
| `TabManager.bookmarkTabs` 与 `liveTab/openBookmark` | 同一固定 ID 最多对应一个 live Tab；首次点击创建，重复点击复用控制器 |
| `TabManager.close` | 固定项关闭只卸载 live Tab；不删除保存项，也不进入普通标签的关闭恢复栈 |
| `TabManager.select` 的 Favorites 规则 | Favorites 在所有工作区共用，同一控制器随用户当前工作区显示 |
| `bookmarkTab` / `favoriteTab` / `unbookmark` | 普通标签、固定项、Favorites 互相转换时保留 live Tab ID 和 WebView2；不重载网页 |
| `resetBookmarkTab` / `separateBookmarkTab` | 返回保存的网址；或将当前网页分离为普通标签，原固定项保留且可以独立再打开 |
| `Library.updateBookmark` | 编辑固定项只修改保存的名称和起始网址，当前网页不被强制导航；下次重开使用新网址 |
| `bookmarkTitle` | 首页尝试从标题分隔符两端提取站点名，深层页面保留完整标题 |
| 固定分屏 | 保存两个固定 ID 的配对，卸载后重新打开恢复两页；Favorites 中共用一个双图标 tile |
| `Tab.site(of:)` / `isSignIn` | 按站点归属处理跨站导航，保留常见二级国家域名规则；登录主机与登录路径留在原页面 |

原生逻辑集中在 `native/src/bookmarks.rs`，`native/src/model.rs` 负责通用会话状态，`native/src/host.rs` 负责控制器生命周期与 NavigationStarting。`src/lib/bookmarks.ts` 为独立前端预览提供相同的状态转换，原生模式仍只发送命令并展示 Rust 广播。

交互入口：标签右键可固定、移到 Favorites、编辑保存项、返回固定网址、分离及卸载；Favorites 支持中键或悬停关闭按钮卸载、右键转移/编辑/移除。拖动标签到 Favorites 或固定区会转换类型，拖回普通标签会取消固定；网格内拖动可调整顺序。命令面板可查找未加载的固定项，键盘循环按 Favorites → 当前空间固定网页 → 普通标签顺序遍历已打开页面。

旧会话没有 `libraryVersion` 时，原 `tabs[].pinned` 自动迁入独立 `pins`，原页面 ID、顺序、空间和已有 Favorites 均保留；旧 Favorites 自动分配持久 ID。迁移只执行一次，不会因固定网页处于卸载状态而丢失固定项。原 SQLite 数据位置和 WebView2 用户目录保持不变。

Windows 适配边界：保存库仍放在原有 SQLite 会话内，不照搬 `Library.json` / `Session.json`；保留现有32项 Favorites 上限、管理表单和空列表添加入口。WebView2 根据用户发起的顶层导航判断跨站，排除宿主主动导航、前进后退/刷新及重定向；它与 WKWebView 的 `linkActivated` 类型并不完全等同，跨站表单也可能触发另开。尚未移植 Swift 的8秒重定向落地地址识别、固定项文件夹、多窗口共享、撤销栈，以及拖到 tile 中央新建分屏的指向判定；已有分屏可以整体固定和转换。

许可证：Apache-2.0。上游链接：[TabManager.swift](https://github.com/ZigaoWang/zigzag/blob/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d/Zigzag/Tabs/TabManager.swift)、[Library.swift](https://github.com/ZigaoWang/zigzag/blob/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d/Zigzag/App/Library.swift)、[SidebarMenus.swift](https://github.com/ZigaoWang/zigzag/blob/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d/Zigzag/Sidebar/SidebarMenus.swift)、[Tab.swift](https://github.com/ZigaoWang/zigzag/blob/5338e99f58aa9a75a0696d5d96ec74353c3b3c7d/Zigzag/Tabs/Tab.swift)。完整版权和许可记录见 `THIRD_PARTY_NOTICES.md`。
