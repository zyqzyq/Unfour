# A2/A3 临时会话基础验证 — 2026-11-10

关联 [A2 #5](https://github.com/zyqzyq/Unfour/issues/5)、[A3 #6](https://github.com/zyqzyq/Unfour/issues/6)，
父任务 [#3](https://github.com/zyqzyq/Unfour/issues/3)。
分支 `codex/bastion-temporary-session-foundation` 从已合并 PR #8 后的
`origin/main`（`42a2a848dba828f2ef87e0381f1e0b0b095de4d3`）创建；初始工作区干净。

**已实现内部基础能力；真实 AccessClient 兼容尚未验证。**
本轮用户明确允许开发不依赖现场格式的部分。#4 → #5/#6、#5/#6 → #7 的原生依赖不变；
没有 A2 → A3 阻塞边；#3–#7 均保持开放，没有新增测试 Issue，也不合并 PR。

## 实现边界

- `ssh-engine/ssh/temporary`：无通用序列化／Debug 的 Rust 请求、Zeroizing 秘密缓冲、
  不透明句柄、16 项有界队列、256 项有效去重 tombstone、自动过期、显式工作区认领和一次 start。
  重投沿用 receipt；用户重新发起用新 receipt。取消、超时和迟到结果不能重新建立目标。
- 临时 runner：复用 russh 与原有单 PTY/shell helper；只读已知主机指纹，未知指纹在同一握手内
  显式确认且仅存内存。独立传输／认证／通道／信任时限，无测试连接、重连、额外通道或后台探测。
- CommandBus/Tauri/command-client：只暴露安全摘要与句柄动作；没有生产凭据注入入口，
  没有 MCP 临时启动／exec/SFTP/Task/Flow 入口。
- `ssh-terminal`：临时标签复用 TerminalPane；独立输入和 resize；关闭／切换工作区清理，
  原有保存连接视图保持挂载且临时视图活动时停止抢占焦点；无日志导出或持久命令历史。
- 冷／热 argv guard：未知形式固定不支持；OAuth 留给现有 consumer；metadata CLI 保持优先级。
  实际参数 parser 未实现，`launch-shape.mjs` 没有进入产品代码。
- [契约](../architecture/bastion-launch-contract.md) 固定所有时限、内存上限和清理规则。
  [现场观察说明](bastion-field-observation.md) 记录工具前提和只带回合成结构的步骤。

当前内部认证机制只实现 typed password request，**不宣称现场必定使用密码**。
没有客户连接、读取保存连接、配置／注册表修改、替换 Xshell、兼容启动器或部署行为。

## 验证结果

| 检查 | 结果与范围 |
| --- | --- |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib` | PASS：135 项（当时 8 项临时基础测试），含既有 host-key、普通会话和 transport helper 回归 |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib ssh::temporary` | PASS：随后增加 runtime 边界回归，共 9 项；覆盖冷／热 typed 投递、无 UI 保留、重投／新请求、原子认领、跨工作区拒绝、关闭竞争、限额、自动过期、信任匹配／变更／超时、零持久化 |
| `cargo test -p unfour-command-bus --features ssh-native --lib temporary_ssh_commands` | PASS：4 项；仅绑定 127.0.0.1 随机端口的一次性 Rust SSH server，合成身份／密码，单登录／单通道、PTY 输入输出／resize、关闭、认证 30 秒／通道 15 秒超时、阶段取消、PTY/shell 拒绝及迟到成功清理 |
| `cargo test -p unfour-ssh-engine --no-default-features --lib ssh::temporary` | PASS：5 项；非 native 构建仍执行队列／自动过期，连接明确拒绝，不假装模拟连接成功 |
| `cargo test -p unfour-ssh-engine --doc` | PASS：2 项 compile-fail，认证请求不能 Debug／序列化 |
| `cargo test -p unfour-app --features ssh-native --lib launch::tests` | PASS：1 项，空参数、OAuth／metadata 保留、未知 URI／会话文件／执行开关拒绝 |
| `cargo test -p unfour-account callback` | PASS：5 项，回调校验、拒绝／重放／PKCE 行为回归 |
| `cargo test -p unfour --features ssh-native --lib update::tests::metadata_cli_keeps_priority_and_validates_arguments` | PASS：1 项，真实临时文件 metadata 导出、缺失／相对／额外参数拒绝、OAuth 不被 metadata 消费；同时编译 desktop native adapter |
| `cargo check -p unfour-app --features ssh-native`；`cargo check -p unfour --features ssh-native` | PASS：共享 Tauri 适配器、最终桌面 native 构建路径与依赖检查 |
| Vitest：TemporaryTerminalWorkspace、TerminalPane、TerminalPane.output、TerminalPane.clipboard、ui/i18n | PASS：最终 5 文件 32 项；包含临时 I/O 不走普通保存连接／历史路径 |
| `pnpm run build`；desktop `tsc --noEmit` | PASS：完整前端构建及类型检查；Vite 既有大 chunk 提示保留 |
| 变更 TS/TSX ESLint | PASS：0 error；TerminalPane 既有长函数 warning 保留 |
| 相关 crate fmt、diff whitespace、公开秘密审计、大文件检查、变更文档链接 | PASS；大文件 0 blocking |
| 真实 AccessClient argv/URI/文件解析、OS 单实例进程存活、Windows 安装包／离线现场链路 | NOT RUN：缺少 A1 实际交接契约及现场一次性授权；本轮没有声称提供兼容安装包 |
| 人工桌面临时终端视觉／交互验收 | NOT RUN：正式构建没有可用的外部 parser 或调试凭据入口；UI 已由隔离组件测试验证，不能替代 A4 |

loopback 测试会在未知指纹确认前断言 server 尚未收到认证；之后只允许
`login → auth → channel → pty → shell → input`，exec/SFTP handler 被调用即失败。
SQLite `total_changes()` 在接收、连接、输入、关闭前后不变，覆盖 connection、host-key、
history、activity、credential metadata 与 outbox 的零写入；临时路径也不调用 SecretStore。
普通 connection 列表无新增项，普通日志接口不能导出临时目标。
loopback server 不执行真实命令，仅回显合成终端输入；这证明产品终端传输闭环，不证明堡垒机兼容或远端 exec 能力。

初次执行的限制／修正：sandbox 阻止 Vite 临时缓存 rename 和 loopback SSH 连接，
普通本机隔离测试环境复测通过；pnpm sandbox 数据库打不开，授权环境完整构建通过。
早期测试暴露普通 keepalive 约 9 秒中断慢通道，已为临时路径关闭并复测真正阶段超时。
确认 russh 的 PTY/shell 方法只发送请求后，临时路径增加同通道成功回复等待，验证拒绝／不回复不会过早标为 connected。
修正过信任测试等待、组件测试清理和翻译插入位置；没有放宽产品校验来通过测试。
最初 `cargo test -p unfour-account types_tests` 匹配 0 项，不算通过证据，已改用 `callback` 真正运行 5 项。

## 最小待补证据

1. 实际末端程序／父子关系，以及其收到的是 argv、URI、会话文件还是其它 IPC。
2. 用全新合成值重建的语法，保留边界、引号、分隔符和编码；区分原始 Windows 命令行与解析后 argv。
3. 身份／认证字段类别、远端入口或本机代理、动态端口关系；整个票据用户名也不能带回。
4. 正常关闭测试标签／退出测试末端后的 helper 时间线；进程依赖未知项单列。
5. 路径配置入口及优先级的已有文档／只读证据；服务器版本如可取得。

见 [可执行观察步骤](bastion-field-observation.md)。不要求结束代理、复用票据或在客户工作会话上探测通道。
