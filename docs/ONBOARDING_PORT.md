# Onboarding 移植

源码基线：[ZigaoWang/zigzag@78edb3201b8ec1383bedf4f2eb3b3832af6c15f4](https://github.com/ZigaoWang/zigzag/tree/78edb3201b8ec1383bedf4f2eb3b3832af6c15f4)，主要参考 `Zigzag/Import/Onboarding.swift` 的 `OnboardingView`、`HelloPage`、`StepPage`、`SitesPicker`、`AdsPicker`、`ReadyPicker`，以及 `WelcomeWindow.swift`、`Design/Brand.swift` 和 `Design/Tokens.swift`。

## 页面与平台适配

| Swift | Rove | 行为 |
| --- | --- | --- |
| hello | 欢迎 | Rove 字标渐显，开始使用；保留自己的品牌，不使用 Zigzag 商标图形。 |
| bring | 数据导入 | 检测 Windows 本机浏览器配置，多选来源及书签／历史，显示真实导入数量、问题和重试；密码单独选择 CSV 导入。 |
| sites | 常用网站 | 十二个候选网站，最多八个；Continue 写入真实 Favorites，Skip 丢弃本步未提交选择。保留自定义网站及已有 ID、缓存图标；取消置顶不关闭已打开网页。 |
| ads | 广告拦截说明 | 保留流程位置，明确说明当前正常显示广告；没有未生效的开关。未移植 Swift/WebKit 内容拦截后端。 |
| ready | 默认浏览器 | 为当前用户注册 Rove 的 HTTP/HTTPS 候选项并打开 Windows 默认应用设置，或暂时不用。显示系统实际默认状态；若两种协议均已属于 Rove，完成时不再打开设置。外部链接接收及单实例转交已接入，最终关联选择仍由 Windows 完成。 |

复用 560px 内容宽度、20px 卡片内边距、22px 卡片圆角、30px 标题、6/22px 进度点、40/44px 主按钮和选项高度。欢迎字标替换成 Rove；网站图标采用本地字形，离线也可选。页面短淡入淡出，欢迎字标有一次揭示动画，系统或应用启用减少动态效果后取消位移和动画。480×400 窗口允许纵向滚动访问完整内容与操作，不产生横向溢出。

## 状态与生命周期

`native/src/onboarding.rs` 与 `src/lib/onboarding.ts` 定义步骤、固定网站白名单和选择校验。Rust 是原生状态权威，IPC 只接受封闭步骤枚举及网站 ID，拒绝重复、未知、超限选择和过期步骤提交；校验成功后再保存。

首次 SQLite 会话不存在时使用 `State::first_run()`：空 Favorites、无预置 pins、一个新标签页，进入 hello。旧会话缺少 `onboarding` 字段时按已完成恢复，不重置旧标签、网站或工作区。每次返回／继续都写入同一会话；中途退出后恢复上次步骤，ready 提交后才标记完成。与 Swift 独立欢迎窗口不同，Rove 在同一原生窗口切换引导和浏览界面，关闭窗口保留未完成状态。

引导期间暂停创建／显示网页控制器，屏蔽浏览器快捷键和无关浏览命令；窗口最小化、最大化、关闭及原生标题栏继续可用。`presented` 握手允许尚未挂载 PageArea 的欢迎界面显示窗口。重新打开引导会关闭 Peek、查找及浮层，已存在的网页保留控制器，完成后恢复会话。

已有用户可从「设置 → 通用 → 关于 → 欢迎引导」重看。Vite 默认继续提供已有演示会话，`/?onboarding` 单独启用首次运行预览；该预览在 `rove.onboarding.preview.v1` localStorage 项中保存进度，浏览器预览不打开系统设置。

## 浏览器数据导入

迁移参考还包括同一基线的 `ImportCoordinator.swift`、`ChromiumImporter.swift`、`FirefoxImporter.swift`、`CSVImporter.swift` 和 `ImportBatch.swift`。`native/src/import.rs` 读取本机来源，`import_store.rs` 负责 SQLite 原子提交，`passwords.rs` 负责 Windows 文件选择与 DPAPI；前端不接收文件路径或密码。

| 来源 | 数据与位置 |
| --- | --- |
| Edge／Chrome／Brave／Vivaldi | LocalAppData 下的 User Data，各配置的 Bookmarks JSON 与 History SQLite。 |
| Opera | AppData 下 Opera Stable 及其配置，Bookmarks 与 History。 |
| Firefox | AppData 下 Profiles 与 profiles.ini 中的路径，places.sqlite 中的书签和最近访问。 |
| 密码 CSV | 浏览器常见的 url／username／password 列，以及 Bitwarden 的 login_uri／login_username／login_password；支持 UTF-8 BOM 和 UTF-16LE，保留备注及 OTP 字段。 |

书签存为来源独立工作区中的固定标签，不受八个常用网站的上限限制；当前没有书签文件夹，文件夹路径保留在名称中。历史保留 Unix 秒时间，同 URL 只更新更近的记录。书签按来源、URL 和完整名称去重，密码按 origin 和用户名去重，已有密码不会被覆盖。读取阶段在后台线程运行；每个来源的书签、历史、加密密码、导入报告和会话在一个 SQLite 事务中提交，写入失败整体回滚。某来源部分读取失败时可保留成功类别，界面报告问题并允许重试。

源数据库以只读方式通过 SQLite backup 读取一致快照，包含已提交 WAL，不修改原浏览器记录。单次每来源最多 20,000 个书签、最近 50,000 条历史；SQLite 快照上限 256 MiB、繁忙超时五秒，书签 JSON／CSV 上限 16 MiB，CSV 上限 50,000 条。非 HTTP(S) 和含 URL 用户凭据的项目会过滤。IPC 只接受先前检测到的来源 ID，不能提交任意文件路径。

Windows 浏览器密码通过用户在原浏览器中导出的 CSV 导入。密码、用户名和备注作为 DPAPI 密文存入 `imported_logins`，不进入前端状态、session JSON 或导入报告。同 origin 顶层登录页会提供「Rove · 填写登录信息」，点击后选择账号才填入，不提交表单；其他 origin、跨 origin 的表单 action 和 new-password 字段不会填入。每页最多提供 30 个账号，入口在两分钟后消失；「设置 → 密码 → 自动填充表单」关闭时立即移除现有入口，后续页面也不注入。OTP 只随密码加密保存，未提供生成验证码功能；这不包含 WebView2 自有密码库的保存／同步迁移。

Vite 预览明确提示无法读取本机数据，只有桌面版执行导入。原生测试通过仅 Debug 编译可用的 `ERA_IMPORT_FIXTURES` 将所有检测重定向至临时来源，不读取用户的真实浏览器资料。

## 验证

`npm test` 验证选择事务、白名单、保留自定义网站和页面身份。`npm run test:e2e` 覆盖全流程、恢复、跳过、上限、键盘、旧会话入口、深色和 480×400 小窗口。`cargo test --manifest-path native/Cargo.toml -- --test-threads=1` 覆盖旧数据反序列化、首次运行、步骤校验和序列化恢复。

`npm run test:native:onboarding` 构建并用隔离临时目录启动真实 WebView2，验证未完成重启、Favorites 落库、完成标记及设置入口；结果在 `test-results/native-onboarding/checks.json`。已有其他原生集成脚本使用 `native-profile-fixture.mjs` 创建旧版本会话，以继续验证各自浏览功能和旧会话兼容；该测试 fixture 需要支持 `node:sqlite` 的 Node.js 22.13+。

`npm run test:native:import` 使用合成 Edge／Chrome／Firefox 和 CSV，在真实 WebView2 验证类型选择、导入落库、损坏来源重试、重复导入、原始时间、重启恢复、DPAPI 密文及登录页点击填充和排除规则。结果保存到 `test-results/native-import/checks.json`。Rust 测试额外覆盖 WAL 快照、profiles.ini、Unicode／引号／换行 CSV、事务回滚和保留更新记录。界面 E2E 的模拟 IPC 只验证选择、忙状态、结果和小窗口布局，实际导入由原生集成覆盖。
