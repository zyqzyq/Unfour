# 堡垒机终端 curl：阶段诊断与恢复现场验证

## 现场证据与本轮目标

用户在上一版 API 包中发送后等待，终端提示“无法确认终端是否已恢复”。不发送 HTTP
的“已登录终端 curl”能力检测同样失败；截图首项为 `TIMEOUT`，未收到退出码，随后
为 `TERMINAL_RECOVERY_REQUIRED`。该提示代表输入被保护性锁定，不等同于 SSH 传输断开。

旧版检测把启动、握手、输入确认和执行都放进相同的 10 秒总限期，无法从截图区分
具体卡点。另已在真实 POSIX PTY 复现：工具预检在收到会话标识前失败时，父 shell
与 tty 已恢复，驱动仍会等待不可能出现的关联恢复确认，最终错误锁定终端。
这证明恢复缺陷存在，但不能把工具缺失或长行限制认定为现场 TIMEOUT 的确定根因。

## 本轮行为

- 保留启动、会话就绪、操作确认、输入确认、执行开始、执行结束和恢复的固定阶段码。
  总超时仍有界，发生时保留实际等待阶段；检测不再仅返回笼统 `TIMEOUT`。
- printf/base64 预检纳入已关联的子 shell 生命周期，失败后被动等待其恢复确认。
  未确认协议就绪时不盲发 Ctrl+C；仅收到匹配会话的恢复确认才解除输入保护。
- 真实断线、未知终端状态、恢复确认缺失仍失败关闭，不假装恢复、不重连或重放请求。
- terminal curl 未设置有效超时（0/空）时使用 30 秒，明确正数保持原值；curl 与本地
  驱动使用同一有效限期。普通直连与 TCP 转发的 0 超时语义保持原契约。
- 本地桌面 IPC 不再因 WebView 的 offline 标志被排队暂停；网络错误仍由执行端返回。
  取消 IPC 失败或返回未取消时退出“取消中”，保留在途请求和再次停止入口，不伪造完成。
- 保留首次错误和已收到的完整 HTTP 响应，恢复提醒不替换原始诊断；不自动重试 POST。

| 固定码 | 排查位置 |
| --- | --- |
| `SSH_TERMINAL_STARTUP_TIMEOUT` | 子 shell 启动响应未收到 |
| `SSH_TERMINAL_READY_TIMEOUT` | 会话就绪或预检握手未完成 |
| `SSH_TERMINAL_ACCEPT_TIMEOUT` | 操作确认未返回 |
| `SSH_TERMINAL_INPUT_ACK_TIMEOUT` | 输入数据消费确认未返回 |
| `SSH_TERMINAL_BEGIN_TIMEOUT` | 命令开始标记未返回 |
| `SSH_TERMINAL_END_TIMEOUT` | 命令结束与完整回传未确认 |
| `SSH_TERMINAL_RESTORE_TIMEOUT` | 终端恢复未收到有效确认 |

这些代码不携带命令、响应内容、凭据或随机协议标识。独立恢复结果不覆盖原始失败。
SSH Task 与本轮 raw curl 使用不同执行协议，Task 成功不代表 curl 路径已通过现场验证。

## 现场顺序

1. 关闭旧程序并替换本轮主程序，从堡垒机新登录目标机，停在空闲命令行。
2. 先运行“能力检测 → 已登录终端 curl”，此步骤不请求业务 HTTP。
3. 如果失败，复制脱敏报告，保留第一条阶段码及恢复结果。若显示恢复未确认，按提示
   关闭标签后重新登录，不在未知状态中输入业务命令。不要仅记录上方通用恢复提醒。
4. 检测通过后，确认原终端仍可交互，再发送获准的 JSON POST。未设置超时时默认
   30 秒；确需长请求时在请求设置中指定足够的正数超时。
5. 验证完整响应、停止请求和再次手工输入。取消/超时不证明远端撤销；结果不确定时
   先核对服务端，不自动或反复重发 POST。
6. 若再次失败，带回阶段码、首次错误、恢复结果及大致耗时即可，无需真实票据或正文。

本轮继续包含此前 60 秒协议保活与安全重新协商修复。真实客户堡垒机的具体卡点仍
需新阶段报告确认；本地合成服务器及 POSIX PTY 验证不能替代现场能力验收。

## 验证与主程序交付

- Rust 原生 SSH：246 项 + 2 项文档测试通过；HTTP engine：135 项通过。
- Command Bus：临时会话 8 项、terminal API 2 项通过，包括普通直连超时配置不受影响。
- 前端定向覆盖 169 项，最后受影响的 61 项复跑通过；desktop TypeScript 检查通过。
  ESLint 无错误，保留 6 条大函数相关告警；大文件检查无阻塞项。
- 真实 Git Bash/winpty POSIX PTY：二进制、大输出、后台输出、取消、noflsh、curl
  工具选项及非法选项 7 个场景通过，均核对 tty 恢复、父 shell 可用和无 payload 回显。
- 新 bootstrap 的缺 base64、非内置 printf 两个真实 PTY 故障场景通过：收到关联工具
  错误与恢复确认，父 shell 可用，无 payload、无 Ctrl+C。原始证据位于
  `target/bastion-api-curl/qa/raw-pty-34114c028b/result.json`。
- 生产代码生成的 curl 命令经真实 PTY 请求本地 HTTP 服务，3 个场景通过：约 70 KB
  JSON POST、超过 64 KB 响应、HTTP 500、chunked 响应、正文/认证精确传递和历史脱敏。
  原始证据位于 `target/api-terminal-curl/qa/http-pty-56bab665d0/`。
- 独立 review 已修正同包 `ACCEPT+RESTORED` 后继续发送、恢复缓存增长及缓存输出绕过
  限期的边界；同包完整 `END+RESTORED` 保留响应。最终跨层 review 无遗留阻塞项。
- `cargo fmt` 与 `git diff --check` 通过。未新增依赖，未改变已有用户的持久化配置。
- 最终 EXE：Xshell / SecureCRT 兼容冷启动、已有实例启动、交叉客户端启动全部通过。
  工具预检失败零 Ctrl+C 且恢复；执行结束标记缺失返回 `SSH_TERMINAL_END_TIMEOUT`，
  仅中断已确认就绪的子 shell，恢复后同一终端继续交互，无再次认证。
- 最终 EXE 的 API JSON、大响应、HTTP 500、重定向、连接失败、手动取消、默认 30 秒
  超时均结束等待并恢复终端；截图已核对原始诊断和超时状态。SSH Task 回归通过。
  两轮均检查临时会话、凭据哨兵、运行记录和能力报告未进入禁止的持久化位置。

本轮集中证据目录：`target/bastion-curl-recovery/20261117-144910/qa/`。

只需携带：

`target/bastion-curl-recovery/20261117-144910/delivery/unfour.exe`

- 版本：0.14.0 / test / Windows x64；大小：74,196,992 字节。
- SHA-256：`2BC14AC5318F62CD57032A31D1E6178EF6ABCA7CEF9FB16128D6390717BA680E`。
- `verification.json`、`SHA256SUMS.txt` 及源码快照位于 `delivery` 的上一层目录。
  构建快照包含 233 个有改动的源码/测试输入；验收快照相对构建快照仅调整两个 QA
  脚本，避免连续检测读取上一轮结果；产品输入完全一致。
- 本轮没有提交或推送。此前的保活与安全重新协商仍包含在主程序中；本轮运行了其
  相关 native / Command Bus 回归，130 秒后台空闲实包证据沿用
  [上一轮记录](bastion-keepalive-2026-11-17.md)，不声称重新做了多日 VDI 验收。

本地验收已通过；现场根因尚需新阶段报告确认。请先按上面的“现场顺序”检测，
通过后再测试获准的 JSON POST。不要将本地合成成功等同于客户堡垒机已验收。
