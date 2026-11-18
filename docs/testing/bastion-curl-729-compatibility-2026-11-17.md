# 堡垒机终端 curl 7.29.0 兼容修复与验证

后续协议、旧版回归和现场验收见[小请求耗时记录](bastion-curl-latency-2026-11-18.md)。

本轮处理现场确认的 curl 7.29.0 兼容缺口：curl 能力检测通过，API 请求页有 JSON 正文，
但通过已登录终端发送后收到 HTTP 400，服务端提示 JSON 在第 1 行第 0 列遇到 EOF；
同接口直连成功。现场已确认目标机版本为 7.29.0，本机真实旧版程序复现了同一故障机制。
修复后的客户堡垒机业务验收仍待完成。

## 根因与此前交付边界

原实现把 `data-raw` 放入 curl 配置流，并通过 `curl --config - --version` 的退出码检查
选项支持。`data-raw` 到 7.43.0 才加入，7.29.0 不支持；旧版配置解析遇到未知选项仅警告并
继续，`silent` 又会抑制该警告。结果是检测可以返回 0，真实请求也能保留 POST 方法和 JSON
Content-Type，却丢掉正文选项，最终收到服务端空 JSON 的 400 响应。
参见 [curl 7.43.0 发布记录](https://curl.se/ch/7.43.0.html)、
[7.29.0 配置解析](https://github.com/curl/curl/blob/curl-7_29_0/src/tool_parsecfg.c#L196-L225)与
[警告抑制](https://github.com/curl/curl/blob/curl-7_29_0/src/tool_msgs.c#L41)。

能力检测通过只表示被检测的工具/通道条件满足，不代表业务 API 已发送或业务成功。
curl 退出码 0 也不等于 HTTP 200；未启用 `--fail` 时，HTTP 400/500 可以正常返回正文和
退出码 0。本产品保留 HTTP 状态码和响应正文供用户判断，不把业务错误包装成成功。
参见 [curl 的 HTTP 错误处理选项](https://curl.se/docs/manpage.html#--fail)。

上一交付 `target/bastion-transport-probe/20261117-165314/delivery/unfour.exe` 解决了传输
确认与分层检测，没有修复这个旧 curl 正文问题。它与前一包的 `terminal_curl/request.rs`
构建输入哈希相同。对应证据为两包 `build-source-snapshot.json` 以及
`target/bastion-transport-probe/20261117-165314/qa/round-source-delta.json`。
本轮不得把此前现代 curl/合成 SSH 的通过结果当作 7.29.0 已兼容的证明。

## 本轮行为与安全约束

- 普通正文改为配置中的 `data-binary` 字面值，包含本次 JSON POST 场景，保留 UTF-8、换行
  和回车。只修改 curl 选项，不给正文增加前缀、不裁剪或重新序列化用户正文。
- 原始正文首字节为 `@` 时仍使用 `data-raw`，且每次发送前严格检测支持情况。7.29.0
  返回 `SSH_TERMINAL_HTTP_CURL_BODY_UNSUPPORTED`（明确“请求尚未发送”），并在业务 HTTP
  发送前停止；绝不让 `data-binary` 把它解释为
  远端文件路径。参见 [data-binary 的文件读取语义](https://curl.se/docs/manpage.html#--data-binary)。
- 静态选项移到 curl CLI，并使用同一份非敏感公共契约做预检；`--version` 位于被验证的
  选项之后，未知选项不能再被旧配置解析容错隐藏。`-q` 保持为第一个参数，禁用远端
  `.curlrc` 干扰；`--proxy ''` 通过 CLI 表达空代理，避开旧配置对空参数的处理。
- URL、认证头、正文仍通过已验证的 shell builtin printf 和 stdin 配置流传递，不进入
  外部进程 argv，不写远端临时正文文件，不要求安装或升级 curl。
- 支持 `--http1.1` 的 curl 在实际请求中显式使用 HTTP/1.1；不支持该选项时，只允许识别
  出的 7.29.x–7.32.x 使用其默认 HTTP/1.1，并拒绝宣称 HTTP2/HTTP3 能力的回移植版本。
  未知版本不会静默放行。现有响应解析器仍只接受 HTTP/1.0、HTTP/1.1，避免现代 HTTPS
  自动协商 HTTP/2 后产生回归。`--http1.1` 于 7.33.0 加入，见
  [官方选项版本表](https://curl.se/docs/optionswhen.html)。
- 远端 `max-time` 使用向上取整的整数秒，本地继续按原毫秒 deadline 限时。默认仍为
  30 秒；例如 1,234 ms 对应远端 2 秒、本地 1,234 ms。7.29.0 不支持小数秒，原来的
  `30.000` 也可能被忽略。小数支持始于 7.32.0，见
  [官方 max-time 文档](https://github.com/curl/curl/blob/master/docs/cmdline-opts/max-time.md)。
- 已登录终端传输检测、逐段完整 ACK、独占 lease、取消和恢复隔离继续保留。工具检测仍
  不访问业务 HTTP；API 每次发送在远端执行严格选项检查，不能靠先前的检测结果跳过。
- HTTP 400、404、429、500 等响应使用通用 HTTP 错误说明并保留服务端正文；只有 401/403
  显示认证设置入口，不再把所有 HTTP 错误都提示为认证问题。

共享契约位于 `crates/unfour-core/src/terminal_curl_contract.rs`，只包含非敏感固定参数和
能力选择片段。请求构建仍由 HTTP engine 负责，终端执行与租约仍由 SSH engine 负责。
没有新增远端安装、自动重登录、自动重试、凭据重放或业务文件访问。

## 真实旧版基线证据

使用 curl 官方归档源代码在本机编译真实 curl 7.29.0。校验归档内原始源文件均未修改，
不是把现代 curl 的版本字符串替换成旧版。以下结果来自同一个回环 HTTP fixture：

| 基线场景 | curl 退出码 | 实际业务 HTTP | 观察结果 |
| --- | --- | --- | --- |
| 原配置检测 + `--version` | 0 | 0 次 | 不支持 `data-raw` 仍假通过 |
| 配置中插入未知选项 + `--version` | 0 | 0 次 | 未知选项未导致失败 |
| 原 `data-raw` POST | 0 | 1 次 | 服务端收到 0 字节正文，返回 HTTP 400 |
| 改为字面值 `data-binary` POST | 0 | 1 次 | 服务端收到完整 21 字节合成 JSON |
| CLI 严格检查 `data-raw` | 2 | 0 次 | 在 HTTP 发送前拒绝 |
| CLI 检查 `data-binary` + `--version` | 0 | 0 次 | 兼容选项通过，未访问 HTTP |

证据目录：`target/bastion-transport-review/curl-7.29.0-qa/`。

- `source.json`：官方来源 `https://curl.se/download/archeology/curl-7.29.0.tar.gz`，
  归档 SHA-256 为 `67dc5b952ac489191b62dbe95b18d336b821649f61404a280186c72e8cd0b9d6`。
- `build.log`：本机旧版编译记录。
- `verify-baseline.py`、`baseline-result.json`：六项真实旧版基线、请求字节数和原始源码比对。
- QA curl.exe SHA-256：`ab853cee010a3962bc185a2babd637372e65215766ab7fb038c684cf8e09803a`。

该旧二进制仅用于本机 QA，构建能力为 **HTTP-only**，不交付或安装到客户目标机。它证明
旧版选项解析与 HTTP 正文机制，不证明现场 TLS、证书、代理、企业 curl 补丁或堡垒机中继
兼容性。此前 `target/bastion-transport-review/curl-config-compat/` 的现代 curl 合成实验
仅为辅助对照；真实旧版结论以上述 `baseline-result.json` 为准。

## 本轮验证状态

本轮在确认真实旧版问题及修复后完成集中验证与独立 review，再构建离线主程序。

| 验证 | 状态 | 证据 |
| --- | --- | --- |
| HTTP terminal curl 单元 | 11 项通过 | `target/bastion-curl-compat/qa/http-terminal-curl-final.log` |
| Native SSH raw terminal | 24 项通过 | `target/bastion-curl-compat/qa/ssh-raw-terminal.log` |
| HTTP 错误 UI | 26 项通过 | `target/bastion-curl-compat/qa/frontend-http-errors.log` |
| CommandBus 临时 SSH/API | 8 项通过 | `target/bastion-curl-compat/qa/bus-temporary.log` |
| 固定错误码序列化 | 9 项通过 | `target/bastion-curl-compat/qa/core-app-error.log` |
| 最终生产命令 + 真实旧/现代 curl + HTTP | 14 项符合预期，含旧故障复现和安全拒绝 | `target/bastion-transport-review/curl-compat-final/result.json` |
| 真实 POSIX PTY 工具检测/分层检测 | 现代 3 项、旧版 1 项通过；精确 tty 恢复 | `target/bastion-transport-review/curl-modern-probe-pty-final/result.json`、`curl-old-probe-pty-final/result.json` |
| 现代 curl + 真 PTY + 生产请求/响应解析 | 3 项通过，JSON、HTTP 500、chunked | `target/bastion-curl-compat/qa/http-pty-final/result.json` |
| 旧 curl + 真 PTY + 生产请求/响应解析 | 70,127 字节 JSON 原样收到、119,135 字节响应、HTTP 200、终端恢复和历史脱敏通过 | `target/bastion-transport-review/curl-old-http-pty-final/result.json` |
| 最终独立 review、格式/类型检查 | 无剩余阻断；fmt、diff-check、tsc、ESLint、i18n 4 项通过；大文件检查 0 blocking | `target/bastion-curl-compat/qa/` |
| 实际 unfour.exe 验收 | Xshell 冷/热启动、SecureCRT 跨客户端热启动、API/Task/检测/取消/默认超时/恢复/脱敏通过 | `target/bastion-curl-compat/20261117-173749/qa/windows-xshell.log` |
| 客户堡垒机原 JSON POST | 未现场验收 | 用户回传结果后补充 |

本轮只交付 `unfour.exe`，不提供完整安装包，不携带 QA curl 二进制。

| 交付身份 | 值 |
| --- | --- |
| 主程序 | `target/bastion-curl-compat/20261117-173749/delivery/unfour.exe` |
| 文件大小 | 74,224,640 字节 |
| SHA-256 | `DC793A6E544E0794D0895E901ECD1488F5D57E0BB7ECDA862A02F831D2431295` |
| 构建 | 0.14.0 test，Windows x64，ssh-native，隔离测试标识 `dev.unfour.bastion.a2test` |
| 基础提交 | `10333c2f468f00093190b0b0b66415e5f162501a` + 工作区改动 |
| 源码快照 | 同目录上一级 `build-source-snapshot.json`，241 输入，构建及 EXE 验收后均未变化 |
| 快照 SHA-256 | `020CD8F4DA104BACDDBA60FC815545814D4EFEDF550242C278E0055F517EB191` |
| 汇总记录 | `target/bastion-curl-compat/20261117-173749/verification.json` |

交付目录仅有主程序。QA、源码快照和验证记录留在旁侧目录；本轮相对前包的 18 个源码/QA
输入变化见 `qa/round-source-delta.json`。本机验证通过不代替下述现场验收。

真实 curl 矩阵还确认：预检没有发 HTTP，环境代理没有被使用，没有重试，敏感正文与
认证值不进入外部 argv；旧版首 `@` 文本以 124 退出，现代版原样发送。所有传输的实际
CLI 均保留整数秒超时，现代版本实际强制 `--http1.1`。首轮新增单元用文本历史快照断言
原始正文而失败（快照按设计脱敏），已修正断言并在上述 final 日志中全部通过。

PTY QA 在终端输出渲染前捕获 stdout，以避开 ConPTY 消费协议控制字节；它覆盖真实
shell/tty 输入和生产 HTTP 构造/解析，但不执行 Rust SSH 驱动，也不验证生产总 deadline。
本机旧版 70 KB 输入的 PTY 过程约 83 秒（恢复 110 ms），不能据此声称生产默认 30 秒
内可以完成同尺寸请求；该证据验证字节完整性，实际容量与速度还受现场往返延迟影响。
驱动的限时、取消、恢复隔离由 native 测试覆盖；现场网络延迟、企业发行补丁、TLS 与
堡垒机中继仍未验证。前轮记录的极早取消恢复不确定时隔离终端行为没有放宽。

## 现场集中验收步骤

退出旧 Unfour，替换本轮主程序，通过原堡垒机进入已登录目标机的空闲 POSIX 命令行；
目标机继续使用现有 curl 7.29.0，无需安装或升级任何工具。

1. 运行一次“已登录终端 curl”能力检测。它自动完成传输往返、传输恢复、curl 工具和
   curl 恢复四项；传输预期退出码 23 是设计值，不是业务失败。
2. 四项通过后，使用原 JSON 正文、原请求头和原地址，发送一次获准的 JSON POST。
3. 对比直连时的 HTTP 状态码和响应正文，确认不再出现空 JSON 的 EOF 错误，并核对
   请求页正文。检测通过和 curl 退出码 0 均不能代替这一步的业务响应判断。
4. 回到同一个已登录终端执行一次获准的 `ls`，确认终端仍可交互。

若任何阶段失败，保留同一次检测报告或 API 状态码、脱敏响应正文和固定错误码；
不要循环重发结果未知的 POST。恢复未确认时遵循界面隔离提示，不向未知 shell 继续输入。
本轮现场验收不要求新增 Flow、多日保活测试或远端安装。
