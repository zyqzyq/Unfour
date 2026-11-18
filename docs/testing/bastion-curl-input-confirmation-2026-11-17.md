# 终端 curl 输入确认与中断兼容修复

后续协议、旧版回归和现场验收见[小请求耗时记录](bastion-curl-latency-2026-11-18.md)。

## 新现场证据

上一轮新包的现场截图为 `SSH_TERMINAL_BEGIN_TIMEOUT`，随后
`SSH_TERMINAL_RESTORE_TIMEOUT`。这证明启动、会话 READY、操作 ACCEPT 已完成；
不能证明所有命令输入已被远端消费，也不能证明 HTTP 请求成功或失败。

旧检测命令为 627 字节，编码后连同结束行共 3151 字节，分成 7 条数据行。
旧协议每 8 条才等待一次输入确认，因此这次在进入 BEGIN 等待前没有任何数据行确认。
现场 SSH Task 的短 `ls` 成功，没有覆盖这段突发输入。

真实 POSIX PTY 的合成对照发现两项可独立验证的兼容性缺口：小容量转发层丢弃突发
输入末尾时可导致 BEGIN 超时；继承的中断字符不是 Ctrl+C 时，驱动的 Ctrl+C 无法
触发恢复。两者组合可重现截图的双超时。单独丢尾、默认中断键时仍可恢复。
这不是对客户堡垒机容量或 tty 配置的现场确诊。

## 本轮修复

- 每段原始输入 48 字节，编码后的单行（含前缀和 LF）最多 242 字节。
  每行确认关联操作标识、行序号和该行编码长度后，才发送下一行。
- 结束行携带预期总行数和总编码长度。远端先校验，再确认输入结束并解码执行；
  本地收到对应确认才进入 BEGIN 阶段。短输入也不会跳过输入确认。
- 截断或错误确认不再继续发送并执行不完整命令；输入/结束确认缺失返回
  `SSH_TERMINAL_INPUT_ACK_TIMEOUT`，相关元数据错误返回固定协议错误。
- 临时子 shell 显式设置中断字符 Ctrl+C，退出时恢复原有 tty 全部设置。
  保留原来的互斥、内存/输出限制、无凭据回显、完整响应保留、恢复确认与输入保护。
- 不自动重试、重连或重放 HTTP；仍默认 30 秒总执行预算，恢复使用独立短预算。
  逐行确认增加往返次数，较大正文或高延迟链路可能需要明确配置更长超时。

取消仍是尽力停止：在 BEGIN 刚回传、执行子进程尚未建立的瞬间发出中断，可能出现
父 shell 延后处理中断、远端命令仍运行的窗口。本地实验已单独记录这种情况，
未将晚到的恢复标记算作 3 秒恢复成功；此时继续保持“结果不确定”与输入保护。
这与本次现场“未收到 BEGIN”不同，也不构成自动重试的依据。

## 现场顺序

1. 完全退出旧 Unfour，替换本轮 `unfour.exe`，从堡垒机重新登录目标主机。
2. 停在空闲 POSIX 命令行，先做“能力检测 → 已登录终端 curl”。此检测不访问业务 HTTP。
3. 通过后，确认终端能继续手工输入，再测试获准的 JSON POST。
4. 若失败，复制脱敏报告，保留首个错误、恢复结果和大致耗时。恢复未确认时按提示
   关闭标签重登；不要在未知输入状态下继续发业务命令或反复重发 POST。

## 验证与交付

本轮证据目录：`target/bastion-input-confirmation/20261117-153840/qa/`。

- Native SSH 全量：251 项 + 2 项文档测试通过；新增输入交付测试覆盖逐段阻塞、
  错误确认、截断、丢结束行及结束计数拒绝。6400 字节命令在原 10 秒预算内约 2 秒完成。
- 前端结果未知分类及两种异步返回路径：44 项通过；desktop TypeScript、ESLint 通过。
- 真实 POSIX PTY：9 个场景通过，使用与产品一致的 LF，覆盖二进制、大输出、取消、
  `intr=Ctrl-X` / `intr=undef` 的取消与原设置还原、curl 工具和非法选项。
- 真实生产 curl：约 70 KB JSON POST、超过 64 KB 响应、HTTP 500、chunked，
  3 个场景通过；正文/认证精确传递、历史脱敏通过。
  证据：`target/api-terminal-curl/qa/http-pty-069f3a1c44/`。
- 13 项真实 PTY/合成 relay 对照符合预期：
  `target/bastion-begin-timeout/relay-703c9149f9/result.json`。
  即时取消窗口另记于 `target/bastion-begin-timeout/cancel-78fb24cc39/result.json`，
  没有计作恢复成功。
- 初轮 native 大输入测试暴露测试服务器未消费 russh Channel 副本，100 条队列填满
  后死锁：停在 D92，随后 10 秒输入超时和 3 秒恢复超时。修复测试服务器消费队列后，
  保留原 2 字节碎片 ACK 与 10 秒预算，D134 全部交付。未增加产品超时或修改 vendor。
  初轮失败与修正后记录均保留，不将该测试问题认定为现场原因。

- Command Bus 临时会话 8 项通过；最终 EXE 的 Xshell / SecureCRT 兼容冷启动、
  已有实例启动、交叉客户端启动通过。JSON POST、约 100 KB 响应、HTTP 500、重定向、
  连接失败、手动取消、默认超时、工具预检失败、执行结束超时均完成收尾与恢复检查。
  同一会话继续交互、无重复认证、SSH Task 回归及禁止持久化哨兵扫描通过。
- 初次 EXE 验收在 API 与恢复场景全部通过后，第二次启动的新标签未在 5 秒内出现。
  记录于 `qa/windows-xshell.log`。未修改程序、脚本或测试限时，随后 SecureCRT 完整
  验收及 Xshell 原样复跑通过（`windows-scrt.log`、`windows-xshell-rerun.log`）。
  未复现原因尚不明确，保留此观察，不将首轮记为全部成功。
- Review、格式、TypeScript、ESLint、源码一致性及 diff 检查完成。没有新增依赖或修改
  本轮以外的产品行为；本轮源码差异清单见 `qa/round-source-delta.json`。

仅携带主程序：

`target/bastion-input-confirmation/20261117-153840/delivery/unfour.exe`

- 0.14.0 / test / Windows x64；74,204,672 字节。
- SHA-256：`51817071CF39F3E21CE1D4B4D153A71431E240A6A930963F5A2EA31F127E5C6A`。
- `delivery` 内仅有此 EXE；上一层为 `verification.json`、`SHA256SUMS.txt` 和
  `build-source-snapshot.json`，235 个改动输入与构建后源码一致。
- 本轮未提交、推送或关闭 issue；#7 仍需现场检测和业务请求确认。

真实客户堡垒机的具体限制仍需现场验证。Flow 与真实多日 VDI 验收不在本轮范围。
