# 分屏宽度拖动迁移

来源：[Zigzag](https://github.com/ZigaoWang/zigzag)，提交 `0f99cac833a9f23e379f617ae99800b5d6e2f19c`，Copyright 2026 Zigao Wang，Apache-2.0。授权和归属见项目根目录 `THIRD_PARTY_NOTICES.md`。

## 对应关系

| 上游实现 | Rove 的迁移 |
| --- | --- |
| [BrowserRootView.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/BrowserRootView.swift) 的 `ResizeHandle.mouseDragged` 直接回调，随后 `layoutSubtreeIfNeeded()` | `native/src/split.rs` 使用同线程 Win32 子窗口接收鼠标输入、捕获鼠标，在 `WM_MOUSEMOVE` 内直接调整两个快照的合成边界。Vue 仅接收比例反馈；松手时才提交真实网页 HWND 与 WebView2 controller bounds。 |
| `minimumPaneWidth = 260`、20%–80% 比例、窄窗口均分 | 原生拖动和 DOM 备用路径采用相同的尺寸限制，单位为 Windows 逻辑像素。 |
| `paneFrames` 向下取整左侧宽度，共享分隔边界 | 对可用宽度扣除分隔间距后取整一次，右侧从同一边界开始，保持总宽度。 |
| [PageCardView.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/PageCardView.swift) 的 `layout()` 在 `withoutAnimation` 内直接设置 `webView.frame` | 松手时先直接提交两侧最终 viewport，再以独立的快照动画交接到新排版；真实 viewport 不参与尺寸插值。 |
| [BrowserWindowController.swift](https://github.com/ZigaoWang/zigzag/blob/0f99cac833a9f23e379f617ae99800b5d6e2f19c/Zigzag/Window/BrowserWindowController.swift) 仅在 `final` 时提交比例 | 拖动过程只合并发送轻量比例反馈，结束时前端再提交完整布局、恢复装饰。沿用 Rove 现有比例生命周期，此次没有新增跨重启的比例持久化。 |

## Windows 平台取舍

侧栏和分屏分隔条共用 `resize_surface.rs`：创建页面时预热并复用同线程合成设备，开始拖动时异步捕获一次网页，由 WIC 解码后上传 DirectComposition；等待截图时先使用原窗口的合成表面反馈。拖动期间只缩放快照，鼠标附近的边缘向后倾斜，真实 HWND 和 Chromium viewport 保持开始尺寸。卡片以临界阻尼弹簧缩至 96.8%，圆角随之增大至 18 CSS px。圆角裁切位于快照内容内层，透视在外层进行，使裁切随快照边缘一起变形。

松手时立即提交最终真实尺寸，解除原窗口的 cloak，让网页在快照下面绘制。透视先恢复一部分，等待新尺寸截图；新旧画面在同一透视父图层上以非线性交叉渐变衔接，再以 cubic ease-out 恢复 1:1 尺寸和原四角。新截图等待最多 240ms，失败时使用最终尺寸的合成表面完成收尾，避免预览停留。系统或设置启用减少动态效果时只使用短渐变。

网页快照内容使用 DirectComposition 高斯模糊，最大标准差为 3 CSS px，并按 DPI 换算。模糊强度复用缩放的提起进度：随拖动弹簧淡入，在新旧排版交叉渐变期间保持，最终随 cubic ease-out 缩放恢复而降至零；快速打断后再次拖动也沿用当前进度。图层先模糊内容，再裁切圆角，最后进行透视；滤镜使用硬边界模式延展边缘颜色，避免透明光晕，保持快照轮廓与外部流体背景清晰。合成设备通过 `DCompositionCreateDevice3` 创建以支持该效果，模糊不触发网页排版或重绘。减少动态效果时不启用模糊。

分屏入场分为两段：收到最终插槽布局时，先捕获原网页的完整尺寸，保留其真实视口，让快照以两端速度和加速度均为零的五次曲线，在 280ms 内连续移动、收缩到左侧插槽。取得快照后开始移动，捕获等待最多 120ms。到位后才提交原网页的最终 HWND／视口，再启动另一侧网页从右侧滑入、淡入。等待期间，另一侧只显示 Space 背景；原快照保持在它上方，避免提前露出第二个网页。

两侧继续使用内容模糊、锐利的 18 CSS px 圆角、透视、新排版交叉渐变与 cubic ease-out 缩放恢复。动画期间不逐帧更新 Chromium 视口。原生层记录已呈现的分屏组合，普通布局报告不会重复播放；命令菜单尚未收起时暂存整个顺序，关闭菜单后再启动。关闭分屏、切换页面或开始拖动可立即中断：已缓存的最终视口只提交一次，待入场快照同步释放，避免遗留冻结网页或背景。缺失截图沿用超时收尾；减少动态效果时直接提交位置，只渐变，不倾斜、缩放或模糊。

快照外使用流体渐变：`resize-appearance.ts` 复用 Space 配色函数，单色 Space 以主题色和邻近色增加可见色差，自定义双渐变保留用户的两个色相、饱和度和方向；按 Space 当前有效明暗模式调整反差，两端均限制在中间亮度。原生层只在开始拖动时上传线性底色与三张带柔和透明边缘的色团纹理，随后以不同相位的非线性路径移动、缩放色团，产生融合流动，复用现有合成时钟，不逐帧重绘纹理或触发网页渲染。减少动态效果模式固定色团位置。切换 Space 或编辑配色时，可信界面通过 `resizeAppearance` 更新原生动画外观。

DirectComposition 的绘制目标可能复用图集旧像素。上传纹理前按 `BeginDraw` 返回的偏移裁切到当前更新矩形，并清为透明；否则半透明色团以 source-over 绘制时会保留旧快照。纯色填充也使用同样的矩形裁切，避免清除相邻图集内容。GPU 读回回归会先写入旧画面，再验证透明上传完整替换目标像素且不影响周围内容。

普通前端布局消息只保留最新一份，尺寸不变时跳过 Win32 和 WebView2 调用。原生拖动开始时丢弃已排队的旧布局；切换分屏组合后拒绝旧组合的比例反馈。关闭标签前先释放捕获和 controller 引用。

窗口缩放时，原生尺寸事件直接保持当前分屏比例和共享间距更新两侧视口，界面脚本繁忙也可继续缩放；布局回报携带来源视口，避免旧窗口尺寸的迟到回报覆盖原生尺寸。侧栏的独立原生输入条在鼠标消息内直接用 WebView2 消息通知界面更新宽度；16ms 定时器仅合并宿主布局状态，显示不再等待该定时器或 winit 事件。拖动只修改侧栏和标题栏局部样式，松手后才提交根宽度变量，避免每帧使整棵界面重新计算继承样式。未取得合成设备时继续使用实时视口路径。DOM 备用输入根据已有几何数据计算增量，避免每次指针移动强制排版。`ResizeObserver` 负责最终布局与圆角恢复。

松手、失去捕获或取消都会提交最终尺寸；切换标签、打开浮层、改变窗口大小与关闭页面会清理对应预览和定时器，恢复原窗口样式及 cloak。分屏两侧独立清理，即使一侧提交失败也不保留另一侧预览。

独立浏览器预览及键盘操作继续使用 DOM 路径。DOM 拖动在输入回调里直接计算并发送两侧尺寸，保留与原生路径相同的限制和收尾。

## 验证范围

- Rust 测试覆盖比例边界、共享分隔边界、原生 HWND 鼠标消息、鼠标捕获与取消、通知合并，以及真实 GDI 区域的清除和恢复。
- `resize_motion.rs` 覆盖透视方向、渐变与缩放的顺序、缺失截图与减少动态效果的收尾；`resize_surface.rs` 使用真实 HWND 验证拖动不改变原窗口几何、超时后恢复样式和 cloak，以及 WIC 解码和 Space 渐变方向。
- `tests/resize-appearance.test.ts` 和侧栏 E2E 覆盖命名颜色、自定义双渐变、有效明暗模式以及配色更新的原生消息。
- `tests/e2e/window-layout.spec.ts` 验证前端尺寸同步、取消拖动、圆角恢复，以及原生反馈期间不反向发送布局。
- `npm run test:native:split` 使用临时 profile 和两个真实 WebView2 页面，检查拖动期间 viewport 更新、页面持续可见、未重新加载、不触发浮层，并采样网页实际输出的 PNG 尺寸。
- `node scripts/native-split-entry-smoke.mjs` 使用独立 profile 和真实 WebView2 检查原窗口连续移动后才引入另一侧、滑入与淡入、交叉渐变、最终视口、重复布局不重播、菜单关闭后启动、移动途中取消，以及减少动态效果；`ERA_SMOKE_EXE` 可指定构建路径，`ERA_SMOKE_REVIEW=1` 在分屏创建前等待桌面检查。
- 原生鼠标输入需另用真实桌面拖动验证；CDP 的 `page.mouse` 只覆盖 DOM 备用路径。`ERA_SMOKE_REVIEW=1` 会在分屏就绪后暂停，供桌面验证；在输出目录创建 `resume` 文件后继续自动检查。

CDP 截图采样会引入开销，不能作为桌面合成延迟或零延迟的证明；复杂网页仍受 WebView2 自身排版与绘制成本影响。
