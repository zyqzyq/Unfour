# 已登录终端 curl：API 主程序现场测试

## 目的与范围

现场 TCP 检测返回不支持，而 SSH Task 复用目标终端已有输出。本轮增加 API 的显式
`terminalCurl` 模式，在用户已经进入的目标 shell 中运行 curl，通过同一 PTY 回传完整
HTTP 响应。原直连与 SSH TCP 模式保留；不会在失败后自动切换模式或重发请求。

优先覆盖 JSON POST（包括 embeddings）。支持 HTTP(S)、常用方法、Query、Header、
现有认证解析及 JSON/文本/urlencoded；沿用 API 的变量、前后脚本、响应展示及历史脱敏。
不支持 multipart 文件、本地证书上传、二进制响应、SSE/WebSocket。首版不跟随重定向，
直接展示 3xx；HTTP 4xx/5xx 保留响应头和正文。

## 现场步骤

1. 关闭旧版 Unfour，备份并替换本次主程序，通过原堡垒机入口重新登录。
2. 在临时终端完成目标机选择和登录，确认当前机器与目录，停在空闲 POSIX shell。
   目标需要 `sh`、`stty`、shell 内置 `printf`、`base64` 和支持所需选项的 `curl`。
3. 在能力检测中选择“已登录终端 curl”，确认目标与空闲状态后检测。此项检查工具、
   选项和编码回传，不发 HTTP 请求；通过不代表目标 API 地址可达。
4. 打开已有 POST 请求，网络路由选择当前临时节点，执行方式选择
   “复用已登录终端（curl）”。检查 URL、JSON 和认证，超时建议先设为 20 秒。
5. 本次发送前确认目标终端空闲，然后点击发送。应在原 API 响应面板看到 HTTP 状态、
   响应头、耗时和完整 JSON。embeddings 数组不应因为超过 64 KiB 而截断。
6. 请求期间该终端暂停手工输入、终端 Task 和终端检测。结束后回到终端，等待占用提示
   消失，再执行一个获准的只读命令，确认原 shell 可用；下一次 API 发送需要重新确认终端状态。
7. 在批准的测试接口检查 HTTP 500、慢响应取消及断线。完整 HTTP 500 应展示正文；
   取消/超时不保证服务端撤销，请核对服务端状态后再决定是否发送。
8. 如果已收到完整响应但终端恢复失败，应保留响应并提示重新连接。不要因恢复警告
   再次发送已经完成的 POST。

只带回脱敏能力报告、固定错误码、HTTP 状态、是否完整和终端是否恢复。不要带回真实
Token、原始请求/响应正文、堡垒机票据或客户目录。主程序中的原 TCP 转发检测仍可使用。

## 错误与边界

| 情况 | 处理 |
| --- | --- |
| curl/工具缺失或选项不支持 | 明确报告，联系管理员准备工具；不自动安装 |
| DNS、连接、TLS 失败 | 分别显示错误；网络与证书环境来自目标机，不自动关闭证书校验 |
| 终端正被占用/需要恢复 | 发送前阻止；等待当前操作完成或重新登录 |
| 超时、取消、断线、回传损坏 | 保留远端结果可能未知的提示，不自动重试 |
| 请求过大 | curl 配置编码后最多 512 KiB，超限发送前拒绝 |
| 响应过大/类型不支持 | 正文最多 10 MiB；超限、二进制、流式或压缩响应明确报错 |
| HTTP 3xx | 原样返回并提示未跟随重定向 |

请求参数通过已关闭回显的协议传递，验证 shell 内置 printf 后由管道输入 curl 配置，
不主动创建远端请求/响应文件，不把请求值写入 curl 外部进程参数。传输编码仅避免
PTY 字节转换，不是额外加密。固定引导行可留在远端 shell 历史；堡垒机审计与目标机
自身记录策略仍适用，不能承诺远端没有记录。

API 历史遵循现有脱敏规则，不等于 SSH Task 的全部结果仅内存策略；临时连接 handle
及执行模式不保存为持久连接。Flow 接入临时会话不在本轮范围。

## 验证与交付记录

SSH 引擎新增对仓库已使用版本的 `base64 0.22` 与 `tokio-util 0.7` 的直接依赖，分别
用于 PTY 原始字节回传解码和共享 API 取消信号；没有新增远端安装步骤。

| 检查 | 结果 |
| --- | --- |
| SSH native / default | PASS：226 + 2 doc / 124 + 2 doc；新增 10 MiB 响应定向测试 1 项通过 |
| HTTP 引擎 | PASS：134 项，含终端 curl 的请求转义、响应完整性、500 与历史脱敏 |
| CommandBus API 回归 | PASS：51 项；最终 ACK 协议接入后 curl 编排再定向通过 |
| AppError 固定码 | PASS：9 项，附加远端文本不能冒充稳定码 |
| 真实 POSIX PTY | PASS：7 项；另外 70,023 字节输入通过，tty 恢复、父 shell 可用、无载荷回显/历史 |
| 真实 curl + HTTP | PASS：3 项，70 KB JSON POST、119 KB 响应、HTTP 500、chunked；每例恰好一次请求，凭据历史脱敏 |
| 前端回归 | PASS：104 个文件、907 项测试；TypeScript 通过；ESLint 0 error、6 项复杂度/长度 warning |
| 最终路由弹窗修正 | PASS：4 项定向测试、TypeScript、ESLint；标题完整显示，有临时会话时不再误提示添加本地连接 |
| 桌面编译与静态检查 | PASS：desktop ssh-native cargo check、Rust fmt、大文件门禁（0 阻断） |
| 最终 EXE 离线验收 | PASS：Xshell / SecureCRT 参数格式各一组冷启动、热启动、交叉热启动；每组含连续 200/500/302/连接失败/取消及终端恢复 |
| 最终 EXE 隔离存储检查 | PASS：两组均无合成凭据落入 DB/日志/WebView profile，临时 SSH/信任/Task 运行/运行活动/outbox 记录为零 |
| 独立代码 review | 已修复完整响应与取消竞态、解码前 builtin 检查，并复核输入 ACK、POST 不自动重发与凭据处理；该复核范围无剩余阻断项 |

真实 HTTP 证据为 `target/api-terminal-curl/qa/http-pty-fd76cd7ebc/result.json`。
最终原始终端协议 SHA-256 为
`fe8ec87ebf5911cc9d7c6aeb03a1c04f46f58bb2ea65391c031ab329774deafc`，
对应 `target/bastion-api-curl/qa/raw-pty-a20b198d30/result.json`。

过程中的失败没有作为通过记录：初版大输入在真实 PTY 缺 BEGIN，证据保留于
`target/api-terminal-curl/qa/http-pty-cce740b1df`；增加每 8 行输入确认后重跑通过。
首轮测试编译/旧文案断言与 QA fixture 格式检查失败已修复重跑。
pnpm 在沙箱内不能打开缓存数据库时，静态脚本改用等价的 node 入口执行。

实包首轮的 JSON 编辑器超时来自 QA 使用旧 `textarea` selector；截图确认本地 Monaco
资源已经加载，改为公开 textbox 角色以兼容 WebView2 的 native EditContext。
随后连续请求曾触发终端恢复警告：本地 SSH fixture 持有 PTY Channel 却没有消费
其接收队列，约 100 个包后阻塞了服务端 Handler。此问题修正在 fixture，不通过放宽
恢复超时来掩盖；主程序在该故障下仍保留完整 HTTP 500 并阻止继续使用未确认恢复的终端。
失败证据保留于 `target/api-terminal-curl/20261117-114153/qa/windows-xshell-second-request-recovery-failure.log`。
最终包验收中的一次手工输入断言早于终端状态轮询更新；QA 已等待可见占用提示消失后
再输入。占用期间输入被丢弃而不会随后重放，符合当前终端互斥行为。

真实客户堡垒机、目标 Linux/curl 版本及网络策略仍需以上现场步骤验证。

## 主程序交付

- 文件：`target/api-terminal-curl/20261117-120935/delivery/unfour.exe`
- Windows x64，`0.14.0`，`test / standard`，仅此主程序；不含安装器、sidecar 或 fixture。
- 大小：74,207,744 字节（约 70.8 MiB）。
- SHA-256：`752B73283AC41AD84A20D27B906FA07133F5AF70228F10CF7957AB4A5E618368`
- 来源：`10333c2f468f00093190b0b0b66415e5f162501a-dirty`，含本轮未提交改动。
  构建时 133 项源码输入记录于同目录 `build-source-snapshot.json`，不能只凭 HEAD 复现。
- 构建命令：`pnpm tauri build:test --no-bundle --target x86_64-pc-windows-msvc --features ssh-native --config D:/project/Unfour/scripts/bastion/offline-tauri.json`。
- 实包验收使用全新数据目录和 WebView2 profile，并阻断外部 HTTP(S)。测试只连接本地
  合成 SSH 服务，不连接客户资产。日志与截图在同目录 `qa/`，校验值在 `SHA256SUMS.txt`。

最终包验收及源码对账以同目录 `verification.json` 为准。编译完成后只调整了 QA 的
终端就绪等待，未改产品代码；构建源码快照和最终 QA 源码快照分别保留。
`qa/windows-xshell.log` 与 `qa/windows-scrt.log` 均退出 0，已核对交付 EXE 的 SHA-256。
