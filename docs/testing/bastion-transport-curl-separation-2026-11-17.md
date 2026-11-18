# 已登录终端传输与 curl 分层验证

本轮延续当前堡垒机分支，保留已登录终端执行 SSH Task 与 API 的实现。目标机不需要安装
trz/tsz、Python 或额外辅助程序；继续使用已有 POSIX shell、stty、内建 printf、base64，
业务请求另需 curl。没有增加依赖、重登录、独立 exec 或 TCP 转发要求。

## 实现取舍

- 参考 [Netmiko 同通道跳转](https://github.com/ktbyers/netmiko/blob/56904f587dce032d090405c37d95c4c8c034bec5/COMMON_ISSUES.md#L35-L95)
  的交互状态同步，保留 Rust 原通道、独占 lease 和随机关联帧。敏感输入不采用普通命令回显。
- 参考 [trzsz 能力协商](https://github.com/trzsz/trzsz-go/blob/v1.2.0/trzsz/transfer.go#L567-L688)
  与确认机制，使用 V2 能力声明、逐段完整内容确认及最终总量确认。不引入远端 helper，
  不照搬 raw 模式的块大小，也不把传输完成当成 HTTP 成功。
- 参考 [Pexpect 终端输入限制说明](https://github.com/pexpect/pexpect/blob/fc8f062518b40bd0862aae870cdedf5d9c0c7fc3/pexpect/pty_spawn.py#L527-L559)，
  保留每段 48 原始字节/242 线缆字节和最多一段待确认。当前初始引导命令仍是单行，
  不能宣称覆盖任意堡垒机的输入长度或审计策略。

本轮为机制借鉴，未复制上述项目源码或增加 Python/Go 运行时。

## 行为

1. 默认入口为“已登录终端传输”。约 1 KB 固定输入覆盖多段发送，精确核对输出中的
   普通文本、NUL、控制字节及非 UTF-8 字节，并验证预期非零退出码 23，再确认终端恢复。
2. “已登录终端 curl”自动先完成上述传输检测，传输与恢复均通过才另起一次 curl 工具检查。
   两阶段持有同一 lease，用户输入、API 和 Task 不会插入阶段之间；不访问业务 HTTP。
3. 传输失败明确显示 curl 尚未检测。工具缺失、选项不支持、输入确认超时、执行开始/结束
   超时与恢复失败分别保留；不再由一次命令虚构“工具”和“回传”两项通过。
4. 输入 ACK 同时核对段序号、长度和完整编码内容，同长度修改也会在最终执行前被拒绝。
   ACK 不进入终端 UI、历史、活动日志或脱敏报告。
5. 引导成功后的临时子 shell 规范化 CR/LF 并跳过空行；完整还原原 tty。BEGIN 移到执行子 shell 中，
   通过专用控制输出发出，用户命令不继承该描述符。取消/超时仍有独立 3 秒恢复预算。

## 集中验证

- Native SSH：261 项 + 2 项文档测试通过。包含 V2 碎片 ACK、同长度篡改拒绝、失败不执行、
  精确输出、恢复未知隔离、两阶段独占与取消；`target/bastion-transport-probe/qa/ssh-native-loopback.log`。
- CommandBus 临时 SSH：8 项通过，含新增检测→API 联测；HTTP curl 单元 10 项通过。
  非 native SSH 编译、Rust 格式及 diff 检查通过；large-files 无阻断。
- 前端 66 项通过，退出码 23 文案补充后相关 8 项复验通过；desktop TypeScript 通过，
  ESLint 0 errors，保留原有大函数复杂度 warning。
- 真实 POSIX tty 输入/shell：15 项在最终集中运行中通过，含同终端连续传输→curl、精确 tty
  恢复、二进制/大输出、原样 LF、引导后 CR/CRLF、INTR X/禁用、inlcr/igncr 与取消；
  `target/bastion-transport-review/pty-v2-accepted/result.json`。
- 真实 curl/本机 HTTP：70,127 字节 JSON POST 与 119,135 字节响应，200/500/chunked 三项
  通过，正文与认证精确匹配、历史无合成凭据；`target/bastion-transport-review/http-v2-final/result.json`。
- 合成中继 16 项符合预期，包含旧协议故障对照和 V2 对截断、同长度修改、错误结束总量的
  安全拒绝；`target/bastion-transport-review/relay-v2-final/result.json`。不是 16 项业务成功。

证据分层：Windows winpty/ConPTY 会消费控制帧，因此真实 shell 脚本在终端渲染前捕获 stdout，
它验证真实 tty 输入、shell 执行和恢复，**不证明完整 Rust 驱动总 deadline 或客户中继行为**。
Rust 驱动和租约由 native 合成 SSH 测试覆盖；实际 EXE 验收也使用合成 SSH peer。

保留的失败与边界：默认沙箱阻止回环连接的首轮 native 全量失败并终止，开放本机回环权限后
原代码 261+2 项通过，未修改产品时限。真实 tty 早期重跑中，立即在 BEGIN 后取消仍有一次
未在 3 秒内恢复（`pty-v2-reviewed/cancel-immediate`），后续成功不能抹掉这一竞态。
未确认时客户端隔离输入、不自动重放，不保证每次取消都能继续使用原终端。

源码已完成独立 review，未发现新增阻断。实际 EXE 的独立传输、curl 四项检测、API JSON
响应超过 64 KB、HTTP 500/重定向/连接失败、取消与默认超时、原终端恢复、SSH Task、
Xshell 冷/热启动及 SecureCRT 交叉热启动均通过。合成哨兵未写入 DB/log/WebView profile。
已检查成功四项报告与传输工具缺失时“curl 尚未检测”的实际截图。

## 本轮交付

仅主程序：`target/bastion-transport-probe/20261117-165314/delivery/unfour.exe`。

- Windows x64 / 0.14.0 / test / ssh-native；74,242,560 字节。
- SHA-256：`A6DA4D5CA777E6BFE3FB455D2AEA428D3878FB3BEFBCA22E5F5BA73F37102841`。
- HEAD `10333c2f468f00093190b0b0b66415e5f162501a`，dirty；237 项源码输入在构建前后及
  EXE 验收后保持一致。相较上一交付共有 30 项 source/test/QA 输入变化，没有新增依赖。
- 同级上层 `verification.json`、`SHA256SUMS.txt` 与 `build-source-snapshot.json` 记录
  精确身份；`qa/windows-xshell.log` 为实际 EXE 验收，`qa/build.log` 为构建记录。
- 构建前已完成相关验证与 review；默认沙箱首次 pnpm 构建因本机缓存数据库不可访问失败，
  以本机构建权限重跑成功，未改配置绕过检查。保留 `qa/build-sandbox-failed.log`。
- `delivery` 目录只有 EXE；未提交、推送或更新 GitHub issue，现场验收仍未完成。

## 现场集中验收

1. 退出旧程序，替换本轮 `unfour.exe`，从堡垒机进入目标主机空闲 POSIX 命令行。
2. 运行“已登录终端 curl”。它会自动依次检查传输往返、传输恢复、curl 工具和 curl 恢复；
   四项通过后再发送获准的 JSON POST。预期退出码 23 是传输检测的设计值。
3. 核对 API 状态码和正文，再回原终端执行一个获准的短命令，验证仍可交互。
4. 若失败，保留同一次检测的脱敏报告即可定位阶段；前置传输失败时不继续重复 API 请求。
   恢复未确认时按提示关闭标签重登，不自动重放结果未知的 POST。

客户堡垒机真实兼容性仍需现场验收，本机模拟不能认定客户限制已确诊。较大正文与较高延迟
会增加逐段往返耗时，总预算仍有限；没有修改直连/TCP 超时语义。Flow 与多日 VDI 验收
不在本轮新增范围。

初始引导仍按产品 LF 发送。Windows Git Bash/winpty 测试宿主将最初引导改为 CRLF 时，
子 shell 在接收关联 ID 前退出；该失败仍保留，未认定为客户堡垒机原因。原样 LF 引导后
再对协议输入使用 CRLF 可以通过，不能把它描述为全链路任意换行兼容。
