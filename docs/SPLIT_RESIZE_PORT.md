# 分屏宽度拖动迁移

来源：[Zigzag](https://github.com/ZigaoWang/zigzag)，提交 `0f99cac833a9f23e379f617ae99800b5d6e2f19c`，Copyright 2026 Zigao Wang，Apache-2.0。授权和归属见项目根目录 `THIRD_PARTY_NOTICES.md`。

## 对应关系

| 上游实现 | Rove 的迁移 |
| --- | --- |
| [BrowserRootView.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/BrowserRootView.swift) 的 `ResizeHandle.mouseDragged` 直接回调，随后 `layoutSubtreeIfNeeded()` | `native/src/split.rs` 使用同线程 Win32 子窗口接收鼠标输入、捕获鼠标，在 `WM_MOUSEMOVE` 内直接移动两个网页 HWND 并设置 WebView2 controller bounds。Vue 仅接收比例反馈，不在原生拖动的尺寸更新路径中。 |
| `minimumPaneWidth = 260`、20%–80% 比例、窄窗口均分 | 原生拖动和 DOM 备用路径采用相同的尺寸限制，单位为 Windows 逻辑像素。 |
| `paneFrames` 向下取整左侧宽度，共享分隔边界 | 对可用宽度扣除分隔间距后取整一次，右侧从同一边界开始，保持总宽度。 |
| [PageCardView.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/PageCardView.swift) 的 `layout()` 在 `withoutAnimation` 内直接设置 `webView.frame` | 两侧原生窗口先更新边界，再更新两个网页的真实 viewport；不使用插值、尺寸过渡或等待下一次前端布局。 |
| [BrowserWindowController.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/BrowserWindowController.swift) 仅在 `final` 时提交比例 | 拖动过程只合并发送轻量比例反馈，结束时前端再提交完整布局、恢复装饰。沿用 Rove 现有比例生命周期，此次没有新增跨重启的比例持久化。 |

## Windows 平台取舍

拖动期间保留两侧网页的实时内容与可见性，不使用截图覆盖，不隐藏 controller，也不打开全屏浮层。暂时取消网页卡片圆角和阴影，并且只在拖动开始清除一次 GDI 区域，避免每次鼠标移动都创建区域、触发整页重绘。松手、失去捕获、取消、切换标签、打开浮层或改变窗口大小时结束拖动并恢复裁切。

普通前端布局消息只保留最新一份，尺寸不变时跳过 Win32 和 WebView2 调用。原生拖动开始时丢弃已排队的旧布局；切换分屏组合后拒绝旧组合的比例反馈。关闭标签前先释放捕获和 controller 引用。

窗口缩放时，原生尺寸事件直接保持当前分屏比例和共享间距更新两侧视口，界面脚本繁忙也可继续缩放；布局回报携带来源视口，避免旧窗口尺寸的迟到回报覆盖原生尺寸。侧栏拖动在指针回调中立即读取并提交布局，`ResizeObserver` 仍负责最终布局与圆角恢复。

独立浏览器预览及键盘操作继续使用 DOM 路径。DOM 拖动在输入回调里直接计算并发送两侧尺寸，保留与原生路径相同的限制和收尾。

## 验证范围

- Rust 测试覆盖比例边界、共享分隔边界、原生 HWND 鼠标消息、鼠标捕获与取消、通知合并，以及真实 GDI 区域的清除和恢复。
- `tests/e2e/window-layout.spec.ts` 验证前端尺寸同步、取消拖动、圆角恢复，以及原生反馈期间不反向发送布局。
- `npm run test:native:split` 使用临时 profile 和两个真实 WebView2 页面，检查拖动期间 viewport 更新、页面持续可见、未重新加载、不触发浮层，并采样网页实际输出的 PNG 尺寸。
- 原生鼠标输入需另用真实桌面拖动验证；CDP 的 `page.mouse` 只覆盖 DOM 备用路径。`ERA_SMOKE_REVIEW=1` 会在分屏就绪后暂停，供桌面验证；在输出目录创建 `resume` 文件后继续自动检查。

CDP 截图采样会引入开销，不能作为桌面合成延迟或零延迟的证明；复杂网页仍受 WebView2 自身排版与绘制成本影响。
