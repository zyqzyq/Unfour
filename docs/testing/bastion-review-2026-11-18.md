# 堡垒机 Task 与 API 缺陷验收（2026-11-18）

审核基线为 `d9001d20`，复核从干净的 `main` 工作区开始，HEAD 为 `431a7992`。
其中 `203d00b0` 已包含工作目录保护、登录提示防护和部分 API 结果判断。本记录对应本轮工作区源码。

## 行为与证据

| 项目 | 判定 | 可观察结果 |
| --- | --- | --- |
| 1. API 隧道错误 | 已有修复，本轮补齐 | 首次目标无效、通道被拒，以及尚未转发请求字节时超时，均可确定未发送。POST 已送达后等待响应超时、后续重定向被拒时，保留远端结果未知提示。执行结果和异常使用同一文案生成逻辑。 |
| 2. Task 工作目录 | 先前已修复，本轮验证 | 原生 exec 请求和临时终端解码后的命令都先执行相同的目录保护。真实 `sh` 在目录缺失时返回非零且正文零输出，在含空格和单引号的有效目录中执行全部语句并保留退出码 7。用例包含分号、换行和 OR 列表。 |
| 3. Task 原始 RS | 本轮修复 | 遇到后一个 RS 时重新识别帧，把之前未成帧的字节交还输出。逐个枚举 SSH 切分位置后仍可读到 STATUS、END 和 RESTORED。原生驱动记录 JSON 与正确的成功或失败状态。真实 PTY 的样本输出及退出码见下表。 |
| 4. chunked + trailer | 本轮修复 | 按 `size_download` 取正文。只有最终头指定 chunked 且尾部全为完整合法的 trailer 字段行时才接受额外字节。当前 API 返回最终响应头和正文，另行校验 trailer。 |
| 5. 大资源 HEAD | 本轮修复 | HEAD 的实际传输不传 `--max-filesize`。普通 GET 保留 10 MiB 正文限制。前置 1xx、最终响应头及 trailer 共用 128 KiB 限额，另有终端总输出预算。 |
| 6. 取消与恢复提示 | 取消组合未复现重复；本轮修复 ACK 超时组合 | 将实际响应面板和终端提示一起渲染。终端取消或输入 ACK 超时后，结果未知均只显示一次。恢复失败时另显示一次重新连接提示，原始阶段错误可见。覆盖正常恢复后的取消视图，以及执行结果和异常两条返回路径。 |
| 7. bootstrap 与登录提示 | 本轮补齐，仍有 PTY 边界 | 原生终端读取器识别到独立登录提示后能拦截首次自动写入，即使界面已读走输出也有效。Task 与 API 在等待启动帧和关联 READY 时都会处理登录提示。实测界面在租约前读走 `Pass`、固定引导写出后才收到 `word: `，两条路径均返回登录提示错误，保持隔离，未发送业务数据或主动恢复输入。 |

## 实际验证

| 验证入口 | 实跑结果 |
| --- | --- |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib` | 270 项通过，2 项默认忽略。覆盖输入 ACK、末段丢失、取消、恢复标记、普通输出、两种执行模式和 API 二进制协议。Windows 默认忽略的工作目录用例另指定 Git `usr/bin/sh.exe` 实跑通过。 |
| `cargo test -p unfour-ssh-engine --lib --no-default-features` | 126 项通过。 |
| 原生 SSH 回环、SOCKS 与 CommandBus API | 六个请求场景覆盖首次目标无效、通道被拒、开通道超时、POST 后重定向被拒、目标读完 POST 后断开以及等待响应超时。另测 SOCKS 已握手但首写迟到。核对实际传输阶段、错误码及前端收到的阶段标记。 |
| `cargo test -p unfour-command-bus --features ssh-native --offline --lib temporary_ssh_commands::tests -- --test-threads=1` | 临时 SSH 全组 9 项通过。首次联测修正了模拟终端匹配 POST 的 curl 参数顺序。提交复核时，受限环境的回环连接在 SSH 建连或信任阶段失败，同一组测试在主机环境重跑 9 项全过。另测 API 取消清理及跳过后置脚本、历史写入，通过。 |
| [真实 PTY 验证脚本](../../scripts/bastion/verify-terminal-pty.py) | 实际 Git Bash PTY 运行 `printf '\036{"ok":true}\n'; exit 0` 和退出码 23 的变体，逐字节核对 BEGIN、RS + JSON + LF、STATUS 和 END。正常结束、取消及启用 noflsh 后取消均恢复原 tty 并返回可交互的父 shell。本次本机记录为 `target/bastion-terminal-task/qa/pty-e838c4124f/result.json`。 |
| `cargo test -p unfour-http-engine --locked terminal_curl -- --include-ignored --nocapture` | 分别选择真实 curl 7.29.0、8.18.0、8.22.0，各通过 13 项。生产构造的命令访问回环 HTTP 服务，生产解析器读取 curl 的实际输出。 |
| [curl 兼容性脚本](../../scripts/bastion/verify-curl-compat.py) | 7.29.0 与 8.18.0 的既有兼容矩阵共 14 项符合预期，含普通正文、首字节为 `@` 的处理、预检零 HTTP 以及代理和重试约束。本次本机记录为 `target/bastion-transport-review/curl-compat-46b17b0035/result.json`。 |
| `cargo test -p unfour-http-engine --lib --locked` | 137 项通过，1 项默认忽略。忽略的真实 curl 用例已在上述三个版本分别实跑。 |
| `cargo check -p unfour --features ssh-native --locked --offline` | 桌面原生 SSH 组合编译检查通过。 |
| Vitest 及桌面类型检查 | 提交复核覆盖整个 API Client 包和 3 个 SSH Task、能力检测测试文件，35 个文件共 417 项通过。桌面 TypeScript 检查通过。 |
| 格式与静态检查 | 受影响 Rust crate 的 `cargo fmt --check`、`git diff --check` 通过。变更前端文件 ESLint 零错误，有 10 条函数长度及复杂度告警。大文件检查零阻塞。 |

隧道阶段以 SSH 流接受第一个非空的客户端写入为界。错误先发生时，代理会拦截迟到的首写。
只要重定向过程中有一跳已转发字节，就保留“可能已发送”。TLS 握手也会产生字节，
此时代理仅凭加密流无法确认服务端是否收到业务 HTTP。

回环服务先发送 `100 Continue` 和 `103 Early Hints`，再发送两块 JSON 和 trailer。
三个版本都返回 HTTP 200 及完整的 11 字节 `{"ok":true}`。两个 8.x 版本在解码后的正文后输出
trailer 行，7.29.0 的该构建仅输出正文。同一 20 MiB 资源上，三个版本的 HEAD 都保留
`Content-Length: 20971520`、正文为空，GET 都以 curl 63 报超限。另测了截断、非法字段、
非 chunked 尾部及超预算的头部。7.29.0 为真实的 HTTP-only 测试二进制。审核所用的
8.14.1 本轮未运行。

复跑版本矩阵时，在 Windows 将 `UNFOUR_QA_SH` 指向 Git 的 POSIX `sh.exe`，
将待测 `curl.exe` 所在目录置于该 shell 的 PATH 首位。上述 `terminal_curl` 命令会
在真实进程用例开始时输出 `curl --version` 的第一行，便于核对选中的构建。

## 终端边界与客户验收

1. 用户确认的是当前目标上的空闲 POSIX shell。界面观察到会话不可用、占用、需要恢复或上下文切换时，相应确认失效。每次 Task 启动尝试都使用一次确认，即使临发前发现会话繁忙，重试也需要重新确认。能力检测断开连接时撤销确认。API 每次发送都需确认。
2. 远端没有通用的“当前前台进程为空闲 shell”信号。无回显的菜单、未完成的命令或密码输入、非标准登录提示，以及从检查到 SSH 写入之间的状态变化，都可能接收到固定 bootstrap。界面轮询也可能错过两次采样之间的短暂占用。固定行无请求内容，但可能被远端当成输入或留在其审计记录。
3. 两种驱动先等启动帧，再交换本次会话标识。收到匹配的 READY 和操作 ACCEPT 才送编码数据。每段 ACK 匹配后才发送下一段，子 shell 校验结束行总数和长度后才执行。若结束行已送达而最终 ACK 丢失，业务可能已执行。关联 READY 前不发 Ctrl-C 或 Q。写入过的终端若未收到本次 RESTORED，保持隔离。尚未自动写入的终端可继续手工登录。
4. 现场使用获准的测试资产，确认主机、提示符和无前台程序。先用已有能力检测检查握手和恢复，再跑只读 Task，核对 JSON、退出码和目录失败无正文副作用。使用已授权测试端点核对带 trailer 的 JSON、带大 Content-Length 的 HEAD 及普通 GET 限制。记录目标 curl 构建、HTTP 状态、阶段码及能否继续手工输入，以验证现场企业发行补丁、TLS 和堡垒机中继的组合。
5. 选择无写入副作用的慢请求，分别验证取消后正常恢复与恢复失败隔离。只在获准的测试会话核对现场登录菜单及超时切换行为。发送结果未知时先查服务端记录，再决定是否重发。
