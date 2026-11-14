# 标签休眠移植（Zigzag TabSleep → Rove）

本次移植参考上游 `https://github.com/ZigaoWang/zigzag` 提交 `626a464`（0.9.0 beta 8）的 `Tabs/TabSleep.swift`、`Tabs/TabManager.swift` 的 `sleepIdleTabs` 与 `Sidebar/NowPlaying.swift` 相邻的休眠视觉规则，以及 `docs/PORT_CANDIDATES.md` 的 P0-3 方案。Windows/WebView2 的落地方式与 macOS 不同，差异在下面逐条写明。

## 源码映射

| Zigzag（Swift） | Rove（Rust / Vue） | 说明 |
|---|---|---|
| `TabSleep` 的常量与 `sleepers(candidates,awake:memory:)` | `native/src/sleep.rs` 的 `Thresholds`、`Candidate`、`sleepers` | 判定改为纯函数，单独可测；阈值与 Zigzag 一致 |
| `TabManager.sleepIdleTabs`（每分钟一次） | `host.rs` 的 `Event::SleepTick`（默认 30 秒；debug 可用环境变量缩短） | 判定只看“最后一次被激活”的时间，与 Rove `Tab.last_activated` 同源 |
| `Tab.sleep()`（拍照后拆 WebView，靠 `interactionState` 恢复） | `Page` 上的 `ICoreWebView2_3::TrySuspend` | WebView2 让页面留在原处并释放渲染进程内存，唤醒不需要重建页面，因此没有 Zigzag 的 `interactionState` 与 `PageState` 拍照步骤 |
| `StagedQueue`（每 0.1 秒 2 个） | `host.rs` 的 `sleep_queue` + 最多 2 个 `TrySuspend` 在途，完成事件驱动下一批 | 避免一次事件循环里拆掉大量页面 |
| `MemoryPressure`（macOS 内存压力通知） | `sleep::pressure()` → `GlobalMemoryStatusEx().dwMemoryLoad` | Windows 没有等价通知，按内存占用率分档（见下） |
| 侧栏图标虚线环 + 悬停卡“Asleep to save memory” | `Theme.css` `.tab-row.sleeping`、`TabItem.vue`、`TabPreviewCard.vue` | Zigzag 的环是均匀细虚线圆环；悬停卡显示“已休眠以节省内存” |
| Settings › Tabs 的休眠开关（Zigzag 对应能耗休眠的开关） | 设置 › 通用 › 标签页 › 后台标签休眠（`sleepTabs`） | 关掉立即唤醒所有休眠页并停止新的休眠 |
| `TabSleep` 不碰正在播放/在屏的标签 | `sleep_candidates().can_sleep`：不可见、非当前、非分屏伙伴、未在播放声音、未加载中 | 另加“最近一次休眠失败 10 分钟内不重试” |

## 参数（与 Zigzag 一致）

| 参数 | 值 | 来源 |
|---|---|---|
| 默认空闲阈值 | 1 小时 | `TabSleep.idle` |
| “拥挤”上限 | 20 个存活页面 | `TabSleep.crowd` |
| 拥挤时阈值 | 15 分钟 | `TabSleep.crowdedIdle` |
| 内存不足时阈值 | 5 分钟 | `TabSleep.lowOnMemoryIdle` |
| 内存分档 | 占用 ≥ 90% 为低、≥ 96% 为严重 | Windows 适配值（`sleep::level`） |
| 失败重试间隔 | 10 分钟 | Windows 适配值；WebView2 `TrySuspend` 可能因在屏/播放/调试而拒绝 |
| 扫描间隔 | 30 秒 | Windows 适配值；Zigzag 为每分钟 |
| 同时休眠 | 最多 2 个在途，完成即继续 | 对应 Zigzag `StagedQueue` 的 2 个/0.1 秒 |

判定顺序与 Zigzag 相同：按“最久未看”排序，普通标签在 1 小时、拥挤（>20 个存活）、低内存（5 分钟）三条规则中任一满足即休眠；Favorite 页面只在“严重不足”时休眠；在屏、播放声音、加载中、分屏伙伴、Peek/预览临时页永不休眠。Rove 目前没有私密窗口与 `KeepAwake` 站点列表，因此没有对应分支。

## 运行行为

- 休眠是**运行时状态**，不写会话、不写 SQLite；重启后所有标签都是醒着的，按需加载。
- 宿主在页面重新显示时调用 `ICoreWebView2_3::Resume` 并清除状态；WebView2 在 `IsVisible=true` 时也会自动恢复，两处都做，保证唤醒立即且状态确定。
- 唤醒不会重新请求网页：页面保留 DOM、滚动位置与表单内容；自动化检查用服务器请求计数与页面标题确认了这一点。
- UI 通过 `sleep`（单个页面 `{id, asleep}`）与 `sleepState`（渲染器就绪时的全部休眠 id）事件获得状态；`sleepState` 在 `ready` 与 `PopupReady` 时下发，供新打开的渲染器恢复虚线环。
- 关闭设置开关时，宿主唤醒全部休眠页（`wake_all_sleeping`）并清空待休眠队列；重新打开后按常规规则继续。
- 正在播放声音的页面不会被休眠；若某个页面在休眠后开始播放（理论上的竞态），`Event::Audio` 会立即唤醒它。

## 与 Zigzag 的差异

1. **保存方式不同**：Zigzag 是“拍照 + 拆掉 WebView + 用 `interactionState` 重建”；WebView2 的 `TrySuspend` 保留页面对象，因此无需 `PageState` 的滚动/输入字段快照。代价是：WebView2 是否真的释放了多少内存由 Chromium 决定，Rove 不显示“释放约 N MB”这类数字（Windows 上按进程归因不可靠，宁可不写）。
2. **不检测已输入未提交的文字**：Zigzag 的 `hasTypedText` 来自它自己的 `PageState` 脚本（只记录 `isTrusted` 输入，排除密码/卡号类字段）。Rove 网页没有 IPC，第一版跳过这个条件；正在输入但尚未提交的后台页面理论上可能被休眠。后续可以在页面里预注入一个只记录输入标记的脚本（不向宿主回传内容）再补上。
3. **没有 Keep Awake 站点列表**：Zigzag 站点面板的“保持唤醒”在 Rove 还没有站点面板，暂不提供；表格里预留了对应概念，等站点面板移植时一起做。
4. **调试构建可缩短阈值**：`ERA_SLEEP_IDLE_SECS`、`ERA_SLEEP_CROWDED_SECS`、`ERA_SLEEP_LOW_SECS`、`ERA_SLEEP_CROWD`、`ERA_SLEEP_TICK_SECS`、`ERA_SLEEP_LOG` 只在 debug 构建读取，用于原生冒烟与手动验证；Release 构建始终使用 Zigzag 的 1 小时/20/15 分钟/5 分钟与 30 秒节奏。
5. **失败不打扰用户**：`TrySuspend` 被 WebView2 拒绝（例如页面正在播放、被调试、或旧运行时）时只记 10 分钟冷却，不弹错误；这与 Zigzag“静默跳过”一致。

## 测试与验证

- `native/src/sleep.rs` 单元测试：空闲阈值、拥挤阈值、低内存/Favorite 规则、必须保持的页面、排序、拥挤时只休眠多余数量、内存分档、debug 环境变量。
- `tests/e2e/tab-sleep.spec.ts`：虚线环与提示文案、唤醒后清除、`sleepState` 恢复、关闭标签后的旧运行态清理。
- `scripts/native-sleep-smoke.mjs`（`npm run test:native:sleep`）：真实 WebView2 上 8 项检查——页面 A 休眠、侧栏虚线环与 tooltip、点击唤醒且服务器没有再次请求（证明是恢复而非重载）、页面 B 再次休眠、关闭设置后唤醒全部且不再休眠、重新打开后继续生效。
- 完整 `cargo test`、`npm test`、界面 E2E 与原生冒烟回归结果见 [验证记录](VALIDATION.md)。
- 已知限制：不检测已输入文字；调试器附着到页面时 WebView2 可能拒绝挂起（原生冒烟刻意只连接受信 UI 的调试端口）；缩略图不会为休眠页面重新截图（页面内容未变，缓存图仍是准确的，Desk 会显示缓存或占位）。

## 后续可做

- 页面内输入标记（Phase 2）：document-start 注入只记录“有未提交输入”的布尔标记，配合 `PageState` 的字段白名单，恢复 Zigzag 的 `hasTypedText`。
- 站点面板的“保持标签页唤醒”。
- 休眠页的缩略图与 `sleepState` 在 Desk 卡片上的提示。
- 统计实际释放的内存（`GetProcessExtendedInfos` 只能到进程级，且同站点共享渲染进程，含义有限）。
