# 堡垒机 A4 能力检测现场测试（2026-11-16）

本轮离线测试包合入 `10333c2` 的终端体验修复，以及临时会话内的手动能力检测入口。
目标是为 [#7](https://github.com/zyqzyq/Unfour/issues/7) 补充现场证据，判断后续 Task/Flow
适配的条件；本轮未开放临时堡垒机会话执行普通 Task/Flow，也不代表 #7 已现场验收完成。

## 包内容与核对

新包放在 `target/bastion-a4/` 的独立目录，不覆盖 `target/bastion-a2/` 的旧包。
普通 ZIP 包含 `unfour.exe`、`unfour-mcp.exe`、合成本机 `bastion-loopback.exe`、
`build-metadata.json`、`source-snapshot.json`、`SHA256SUMS.txt` 与本文；完整 ZIP
另带已有 WebView2 x64 和 VC++ x64 离线安装器。现场不需要 Node.js、Rust 或联网下载。

本轮代码尚未提交时，构建标识包含 `-dirty`；`source-snapshot.json` 记录基线提交及
本轮源文件摘要，不能把基线 SHA 当作新入口已经进入该提交的证明。交付时以实际包元数据、
源码快照和外层 ZIP `.sha256` 为准。先核对 ZIP 的 SHA-256，再解压到新目录。
保留原末端映射和旧包；配置变更沿用现场已经验证过的 scrt/Xshell 映射方式。
包使用独立测试应用标识 `dev.unfour.bastion.a2test`，不替换正式安装程序。

仍从堡垒机网页 → AccessClient 发起，选择已经配置为 Unfour 的末端。
不要直接复用 URL、临时用户名/密码或授权票据重试。`bastion-loopback.exe` 只供开发机
合成回归，不模拟真实堡垒机策略，无需在客户环境运行它。

## 先验证终端体验

1. 退出本测试包的旧进程，从网页为一次性测试资产取得新授权并冷启动。
   标签应显示节点名称及地址/端口；名称与地址以启动材料为准，现场核对实际目标正确。
   临时会话不再要求额外点击首次信任；同一临时连接检测到主机指纹变化仍应阻止连接。
2. 在 SSH 终端执行经现场批准的无害命令，确认输入/回显正常；不要用终端输出推断
   native exec 或可靠退出码已经可用。
3. 首次复制合成文本，再从终端右键粘贴。真实系统弹窗若出现，明确允许后，应粘贴一次，
   不应先报错、重复输入或把文本发到另一个标签。记录弹窗是否出现、选择与结果。
   另测 Ctrl+V；用不带换行的无害文本，避免未确认就执行命令。
4. 光标默认约 700ms 显示、700ms 隐藏（完整周期 1.4s）。与普通 SSH 终端比较；
   若系统开启“减少动画”，停止闪烁属于预期。不要用远端不同 cursor 样式误判频率。
5. 保持应用已打开并切到 API 页面，再从网页新授权启动一次，确认自动切回 SSH、
   标签正确且仅消费一次。关闭临时标签后不应自动重连或恢复历史会话。

## 手动能力检测

只在已获授权的一次性测试资产上操作。连接完成后，在临时会话工具栏点击“能力检测”。
打开窗口本身不发送检测命令；每次选择一种检测并确认后，才点击“开始检测”。
所有检测复用当前已经认证的连接，不保存凭据、不发起第二次 SSH 登录。

| 检测 | 执行内容 | 结果的准确含义 |
| --- | --- | --- |
| exec/退出码 | 两个顺序 native exec channel；固定 `printf` 输出合成 stdout/stderr 标记，分别退出 0 和 23 | 两次标记、stderr 分流及 exit-status 符合预期才通过；依赖目标提供 POSIX shell/printf，失败不能直接认定 SSH 不支持 exec |
| 并发通道 | 保留原 PTY，同时打开两个额外 session channel，执行固定合成命令 | 仅证明这一次连接可同时承载这些 channel；不证明无限并发、长期稳定或连接生命周期内无总数限制 |
| SFTP 往返 | 独立授权后，在 `/tmp/unfour-a4-<随机UUID>` 独占创建小文件，写入固定合成内容、读回校验、关闭并删除 | 仅证明当前连接的 SFTP subsystem 与该目录本次往返可用；不证明 WinSCP 独立启动入口或其他目录权限 |

SFTP 需要额外勾选创建、读取和删除测试文件的授权。入口不接受自定义路径或命令。
固定内容为 `unfour-a4-sftp-probe` 加一个换行，共 21 字节；SHA-256 为
`f3663437fee2c853acdf64ae141437a7ed6784217c0e5ed536f1ab61ca5a2f22`。
实现按字节比较并核对精确 EOF；往返 `OK` 表示完整匹配，不靠文件名或大小推测一致。
如果 `/tmp` 不适用或无权限，记录实际结果，不要临时改为业务目录。
点击“停止检测”会取消当前检测；断线/超时/取消后可能无法确认远端清理。
若报告包含清理未确认及随机测试文件路径，应由现场授权人员检查**该精确路径**；
不得批量删除同前缀文件。没有清理成功证据时不要记录“已清理”。
断线后仍可查看和复制已有报告，但不能开始新检测。报告只在内存中保留，显式关闭
临时标签/工作区或退出应用会释放；请先复制需要保留的结果。会话队列上限为 16，
带报告的终态会话应在取证后关闭，避免占用后续启动名额。

保持同一个检测窗口，依次完成三项后用“复制脱敏报告”带回结果；窗口内保留每项最近结果。
切换模块/标签后，前端汇总会释放，重新打开只能取得引擎最近一项结果，请在切换前复制。
报告仅包含应用构建信息、
检测状态和固定结果码等允许字段，不应包含资产名称、IP、用户名、密码、原始命令输出。
不要另外带回原始启动参数、URL、票据、终端日志或含客户信息的截图。

## #7 仍需单独完成的现场项目

- 独立 WinSCP/SFTP 启动入口、认证材料结构、一次授权的文件传输与清理。
  同连接 SFTP 检测不能替代该项。
- 第二次从网页重新授权并启动：记录前次会话是否仍存活、新会话能否进入正确资产。
  **两次新授权不等于同一凭据可重复登录**；没有独立授权的复用实验时保持未知。
- 真实授权到期、AccessClient/helper 生命周期：记录到期前后、正常关闭末端后、批准的
  受控断线后，连接状态、helper 是否退出、资源是否释放，以及重新从网页授权的表现。
  本机 fixture 的连接超时不能代替真实 helper/授权到期证据。
- 连续/并行新启动、工作区切换/关闭、取消与断线，确认所属工作区、标签与资源清理正确。

只有现场能力证据齐全后，才能决定 Task/Flow 如何适配、哪些功能继续禁用；
#3 与 #7 不因合成测试或某一项 probe 通过自动关闭。

## 回传模板

只用测试环境别名；每项填 `PASS` / `FAIL` / `UNSUPPORTED` / `NOT RUN` /
`INCONCLUSIVE`。明确协议拒绝或已知产品边界才可写 `UNSUPPORTED`；超时、断线、
无结果、未授权及缺少适用 shell/目录不能自行推断“不支持”。

```text
日期 / 环境别名：
包文件名 / ZIP SHA-256：
应用版本 / channel / commit（来自 build-metadata）：
AccessClient 版本 / 末端入口 scrt 或 Xshell：
冷启动、热启动、节点核对：
首次复制/粘贴（真实权限弹窗与选择）：
光标 / 标签关闭 / 工作区切换：
exec、0/23、stdout/stderr：
两个额外并发 channel：
同连接 SFTP / 清理：
独立 WinSCP 入口：
两次新授权（不等价旧票据复用）：
授权到期 / helper退出 / 断线与重新授权：
附：复制的脱敏检测报告；未完成项及原因。
```

## 本地交付验证记录

最终交付目录：`D:\project\Unfour\target\bastion-a4\capabilities-20261116-155649`。

| 包 | 字节数 | SHA-256 |
| --- | --- | --- |
| [普通 ZIP](../../target/bastion-a4/capabilities-20261116-155649/Unfour-Bastion-A4-win-x64-20261116-155649.zip) | 44,231,868 | `42b07b8af9d77f63ad8d638b1a242bb2d610bbf305e0b0cfb2c9918fc7ee8545` |
| [完整 ZIP（含离线运行库）](../../target/bastion-a4/capabilities-20261116-155649/Unfour-Bastion-A4-win-x64-20261116-155649-full.zip) | 277,805,095 | `7f87d00cc08307925a86c4213fe4a5d8a0a57a1007440ff559647b3f654f9519` |

元数据确认：版本 `0.14.0`、`test`/`standard`、
`10333c2f468f00093190b0b0b66415e5f162501a-dirty`。
基线提交包含此前 UX 修复，本轮能力检测源码尚未提交；27 个构建相关输入的 SHA-256
已冻结并在交付前逐项复核。文档不参与二进制构建快照，包内保留稳定 README 与现场步骤，
本段最终验收记录及 `qa/verification.json` 留在包外，避免 ZIP 哈希自引用。

| 验证 | 结果与范围 |
| --- | --- |
| 原生引擎 temporary 回归 | PASS：35 项，含 exec/SFTP、取消/超时/迟到响应/清理与报告保留 |
| CommandBus 原生回归 | PASS：6 项；非 native `cargo check` PASS |
| 新入口前端回归 | PASS：4 文件 25 项，含确认、报告脱敏、断线只读与父组件关闭竞争；TypeScript、ESLint PASS |
| 已有终端/剪贴板/i18n 回归 | PASS：5 文件 45 项 |
| 实际 Windows release | PASS：完整 TypeScript/Vite、MCP sidecar、desktop `ssh-native` 构建；合成 fixture release PASS |
| 最终 ZIP 解压校验 | PASS：普通包 7 个清单文件、完整包 9 个；两份 ZIP 外层 SHA-256 与源码快照独立复核 PASS |
| 普通包实际 UI | PASS：Xshell 冷启动、Xshell 热启动、scrt 热启动 |
| 完整包实际 UI | PASS：scrt 冷启动、scrt 热启动、Xshell 热启动 |
| 能力检测 | PASS：总计 6 个认证会话、18 次手动检测；每会话验证 exec 的 0/23、两额外并发通道、SFTP 创建/写入/读回/精确 EOF/关闭/删除。无额外认证，检测后原 PTY 仍可回显 |
| 体验与清理 | PASS：节点/端点标签、无额外首次信任按钮、1.4s 光标完整周期、首次权限交接重试且只粘贴一次、关闭标签、断线后报告可复制且禁止新检测；复制内容白名单与构建信息一致 |
| 持久化与离线边界 | PASS：隔离数据库中 SSH 连接、主机信任、终端/命令历史、Task、SSH 活动、outbox 均为 0；DB/日志/WebView 文件不含合成凭据、节点、URL、probe runId/路径。应用外部 HTTP(S) 被拒绝 |
| 视觉与最终审查 | PASS：实际 WebView2 合成完成态弹窗文案、结果、随机路径和底部按钮完整可见；无未解决阻塞问题。格式、diff、secret audit（2080 文件）及 large-files（0 blocking）PASS |

实际构建命令：

```powershell
pnpm run tauri build:test --no-bundle --target x86_64-pc-windows-msvc --features ssh-native --config D:/project/Unfour/scripts/bastion/offline-tauri.json
cargo build --locked --release --target x86_64-pc-windows-msvc -p unfour-command-bus --example bastion-loopback
node scripts/bastion/verify-windows-package.mjs target/bastion-a4/capabilities-20261116-155649/verified-normal xshell
node scripts/bastion/verify-windows-package.mjs target/bastion-a4/capabilities-20261116-155649/verified-full scrt
```

构建与验包日志保留在包外 `qa/`。过程中的失败与修正：初轮沙箱 SSH fixture 握手受限，允许本机
回环连接后 35 项通过；初轮 `pnpm` 无法打开自身本地数据库，同命令在正常权限下构建通过。
fixture 首编译因 `OpenFlags` 不实现 `PartialEq` 失败，改为比较 `bits()` 后 release 复编通过，
未改动生产应用二进制输入；格式修正后复验通过。构建仍有既有 sqlx lifetime、Vite 大 chunk、
static VC runtime 弃用提示，无构建失败。环境限制和开发 fixture 编译修正不代表现场产品失败。
Tauri CLI 例行把 `tauri-build` 依赖改写为空 `features` 数组，交付前已恢复等价的原格式，
未改变依赖版本、启用功能或构建产物。

合成 fixture 只接受固定 exec 命令，SFTP 只操作内存；不会执行系统命令或提供磁盘文件。
剪贴板权限交接为合成模拟，真实 Windows 权限弹窗、干净系统依赖安装、整机断网、真实
堡垒机策略及 helper/授权到期仍为 `NOT VERIFIED`，按上面的现场步骤取证。
#3/#7 保持开放；本轮未提交、推送或改写 GitHub issue。
