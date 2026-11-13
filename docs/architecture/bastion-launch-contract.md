# 堡垒机外部连接：内部基础契约

外部 SSH 启动会在工作区布局就绪后打开 SSH 模块。只有一个可用工作区时，界面将请求认领到该工作区并开始连接。多个工作区时，由用户在目标工作区确认。两条路径共用主机指纹校验。临时终端显示期间保留普通 SSH 连接树，选择已保存的连接可以返回普通连接视图。

当前工作区已连接的堡垒机会话可供用户显式选择为转发来源。选中后，目标服务通过这次已认证的传输建立 `direct-tcpip` 通道，目标域名由堡垒机解析。普通 SSH 目标仍独立验证自己的主机指纹并使用自己的凭据认证。拒绝转发会结束本次操作。关闭父标签或工作区、父会话断开或重新协商密钥都会关闭所属转发；经临时会话建立的子终端断开后需由用户重新连接。

子终端的复制和手工重连沿用原会话的路由。子终端历史仅记录它曾依赖临时会话，父句柄只保存在内存。重启后必须重新选择父会话并建立新连接。桌面 SSH 任务同样可使用本窗口为目标选定的临时路由，任务的命令和文件传输在目标自己的 SSH 登录上执行。每个父会话容许 32 个尚未确认的转发请求；已取消的通道若收到迟到确认，会直接关闭。

## 2026-11-11 Windows 外层调用

Windows 桌面在 metadata CLI 之后、插件初始化之前校验已拆分的 argv。支持现场观察到的四个参数 `-url <SSH URL> -newtab <标签>`。适配器仅从 URL 构造 Rust 临时请求，校验后丢弃标签。Tauri 主实例在 setup 中接收冷启动请求，已有实例回调使用已准备的 CommandBus。每次真实进程调用分配一个 receipt，UI 就绪前请求沿用 Rust 队列的保留期。

Windows 更新器和应用重启只继承可执行文件名。启动参数中的堡垒机票据和 OAuth 回调由当次进程消费，安装器拉起的新进程按普通桌面启动。

当前单实例插件在 Windows 上以 `|` 拼接 argv，故发送前检查边界，主实例在接收时再次校验。错误只输出固定码。URL 的支持子集、复现步骤与 A4 待确认项见[Windows 离线测试说明](../testing/bastion-xshell-url-offline.md)。以下记录描述 PR #9 时的基础契约。

## 2026-11-10 A2/A3 实现状态

PR #8 已合并。本轮在用户明确授权下实现与现场格式无关的基础能力；
**实际 AccessClient 入口仍不支持，现场兼容未验证**。
见 [基础验证报告](../testing/bastion-temporary-session-foundation-2026-11-10.md)。
本节记录 PR #9 时的临时会话基础实现。Windows 外层入口的后续验证见 [A2 打包实测](../testing/bastion-xshell-url-a2-2026-11-11.md)。

`ssh-engine` 拥有临时队列、认证、归属、主机信任和 runner；CommandBus 编排；
`unfour-app` 只装配入口 guard 和句柄命令；`ssh-terminal` 复用 TerminalPane。
没有在 app-shell 放业务状态，也没有新增 MCP 凭据注入入口。

| 对象／操作 | 当前契约 |
| --- | --- |
| LaunchReceipt | Rust 随机接收 ID + 单调时钟截止时间；同一交接重投复用 receipt，用户再次发起创建新 receipt，不按凭据内容哈希去重 |
| TemporaryRequest | endpoint、整个 username、password 的 Rust 内部类型；无 Debug/Clone/Serialize/Deserialize；字符串使用 Zeroizing；密码机制不代表 AccessClient 的实际格式 |
| 安全摘要 | 随机 handle、可选工作区 ID、固定状态、剩余秒数、固定错误码；信任等待时仅附 SHA-256 指纹；没有端点、用户名、端口、原始 URL 或外部标题 |
| claim/start | 检查工作区存在且未删除，原子绑定，重复 claim 返回当前状态；start 一次取走认证材料，重复 start 拒绝；同一 handle 标识临时请求／目标生命周期，与普通 connection 查找表完全分离 |
| 输入边界 | 只有 typed Rust 接收函数；合成注入只存在于自动化测试。没有 UI/MCP/CLI/环境变量/文件注入 API |
| OS 冷／热启动 | 只分类包络；未知 argv/URI/文件格式输出固定 BASTION_LAUNCH_UNSUPPORTED，不输出原始参数、不执行、不套用上游模板 |
| 既有入口 | unfour:// 仍交 account consumer 校验；build-metadata CLI 仍优先处理；分类不表示 URL 已被账户模块接受 |

最多保留 16 个请求／目标（含活动会话），请求字符串总长度最多 64 KiB。
receipt 和未消费请求有效期为 120 秒；最多 256 个有效去重 tombstone，满时拒绝新请求，
不提前淘汰导致重放。过期 receipt 永久拒绝。弱引用定时器独立清理，不依赖 UI readiness。
UI 单飞拉取摘要；未认领时不连接，通知不作为存储。

| 阶段／事件 | 当前行为 |
| --- | --- |
| 传输／主机握手 | 总上限 150 秒，包含最多 120 秒的显式未知指纹等待 |
| 认证 | 独立 30 秒上限 |
| 初次终端启动 | 建立一个 PTY shell，总限时 15 秒。只有用户随后选择转发时才会发起额外的目标 TCP 通道。 |
| 信任 | 首次握手比对工作区已有指纹，遇到变更即拒绝。未知指纹经用户确认后仅保存在会话内存。当前 SSH 驱动只在首次握手时提供主机公钥，临时会话在重新协商密钥（rekey）时关闭，并提示用户从堡垒机重新发起连接。 |
| 取消／关闭标签 | 移除目标、取消等待、关闭已取得资源；迟到 transport/channel 不能恢复目标；输入/resize 上限 5 秒，关闭 channel/transport 各最多 2 秒 |
| 断开 | 提示从堡垒机重新发起，不重连、不复用材料；临时传输关闭普通 keepalive 探测，避免其 9 秒失活策略抢先覆盖阶段超时 |
| 工作区切换／关闭 | 离开该工作区的 SSH 容器时关闭所属临时目标，永不迁移；未认领请求保留原 TTL；StrictMode 即时 readiness 重放不会误关闭 |
| 删除工作区 | CommandBus 删除后清理目标；workspace epoch 拒绝与关闭同时发生且仍在等待 DB 的迟到认领 |
| 应用退出 | 释放本进程资源，不结束 AccessClient-owned helper，不保存恢复记录 |

以上时限是本地资源上限，不能视为堡垒机授权有效期。单实例第二进程退出是否破坏代理仍未知。
实际冷／热启动接入与 Windows 打包验收不能由 typed 队列测试代替。

临时路径不调用普通保存连接、keychain、session/history、activity、Cloud Sync/outbox、
Task/Flow 或日志导出路径。临时标签无日志导出、SFTP、duplicate/reconnect 控件。
终端可见输出可能含远端回显，仅存有界内存：Rust 待拉取尾部最多 512 KiB；
React 每标签约 256 Ki 字符（单块可能达 512 KiB）；xterm 有限滚动区。
关闭即删除；断开后的 Rust 摘要／输出最多再保留 120 秒并独立定时清理。
不承诺清零 OS argv、JS 或第三方库的副本；Zeroizing 仅覆盖所拥有 Rust 缓冲。

新增直接依赖 serde 用于安全摘要、zeroize 用于秘密缓冲；tokio 从可选改为必需，
确保非 native 构建也会定时清理。版本均复用现有锁定依赖；test-util 只用于确定性时钟测试。

原生依赖关系保持 #4 → #5/#6 → #7，A2 不阻塞 A3。
真实 parser、兼容启动器与 A4 仍需 [现场观察材料](../testing/bastion-field-observation.md)。
`launch-shape.mjs` 仅为证据脱敏工具；Task、Flow、K3s、终端内 exec 回退均未实现。

## A1 原始设计草案（历史上下文）

关联 [A1 #4](https://github.com/zyqzyq/Unfour/issues/4)、
[A2 #5](https://github.com/zyqzyq/Unfour/issues/5)、[A3 #6](https://github.com/zyqzyq/Unfour/issues/6)。
这是待真实末端语法补齐的设计约束，**尚未实现、不是 main 已有 API**。
[本机证据与缺口](../testing/bastion-accessclient-a1-2026-11-10.md) 是启用入口的门槛。

## 边界与所有权

```text
AccessClient 保持现有浏览器入口与授权/代理职责
  → desktop adapter 接收外部输入（Windows argv 边界保留，不经 shell 重拼）
  → CommandBus 校验工作区与受控动作
  → ssh-engine 临时请求/目标服务（Rust 内存）
  → 现有 SSH driver 与主机信任检查

ssh-terminal UI ← command-client ← 安全摘要与不透明句柄
```

`unfour-app` 负责冷/热启动投递与 UI readiness 的组合；不承载 SSH 业务状态。
`ssh-engine` 拥有待认领认证材料、临时目标、连接取消与生命周期。
`unfour-command-bus` 统一编排；Tauri/MCP 不复制域逻辑。
前端业务归 `packages/ssh-terminal`，共享契约归 `command-client`/`workspace-core`；
app-shell 只装配布局。新增 UI 文案必须使用共享 i18n。
本阶段不暴露 MCP 任意外部启动入口，不允许把启动输入转成 Task/Flow 或命令执行。

## 最小内部请求（语义草案，不是序列化结构）

| 部分 | 内容与约束 |
| --- | --- |
| 来源 | Rust 解析后固定枚举；不能接受任意 UI 传来的可信来源声明 |
| 接收标识 | 每次真实进程入口分配随机不透明 ID；相同 URL 再次启动仍是独立请求 |
| 待认领数据 | 目标端点、认证身份（整个用户名也视为秘密）、认证材料、接收时间/本地截止时间；仅 Rust 内存，无通用 Serialize/Debug |
| 安全摘要 | 不透明 request handle、固定来源、固定“堡垒机临时连接”类标签、状态与本地过期提示；不含真实主机、用户名、外部标题、原始 URL、argv 或端口 |
| 归属 | 未选择工作区时保持未认领；认领时显式传 workspace_id 并验证存在/可访问，绑定后不可迁移 |
| 动作 | 只能请求一次交互式临时终端；不能携带命令、启动脚本、Task/Flow、SFTP 自动打开、测试连接或额外探测 |
| 消费 | 原子 claim(request_handle, workspace_id) → temporary_target_handle；重复 claim 返回固定状态，不重复连接 |
| 目标 | 临时随机 ID 与 workspace_id；与普通 connection_id 类型/查找路径分开，不能由保存连接接口反查 |

认证变体由真实启动形式决定。当前不能规定“必定密码”、RFC URL 分隔规则、
解码次数、会话文件读取或本地代理连接参数。未知/歧义/重复关键字段与额外执行开关应明确拒绝，
不得静默截断、选择第一个 URI 或把未知字段传给 shell。
请求摘要使用白名单构造，禁止“先序列化真实连接再删密码”。
所有失败返回固定英文稳定错误码，UI 本地化解释；不透传 SSH/server/解析器原始错误文本。

## 投递、时限与清理

以下是建议的本地资源上限，需在 A2/A3 实现与测试中固定；不宣称等于服务器授权有效期：
最多 16 个待认领请求，每个最多 64 KiB 输入，接收后 120 秒过期。
认证/传输建立总上限 30 秒，通道建立上限 15 秒；主机信任等待受独立 120 秒上限约束。
超限拒绝新请求，不挤掉旧请求、不写磁盘、不为了判定有效期发起连接。

| 事件 | 必须满足的结果 |
| --- | --- |
| 冷启动、已有实例、UI 尚未就绪 | 统一 Rust 队列接收；UI 拉取安全摘要。通知仅提示刷新，不能充当唯一存储 |
| UI 重挂载、重复 readiness、单实例转发重投 | 同一接收标识只消费一次；禁止 URL 内容哈希去重造成真实重复启动丢失 |
| 未选工作区 | 不连接；显式选择后 claim。切换工作区不自动认领、不暗改归属 |
| 用户取消/超时 | 原子终止状态；取消正在等待的认证/通道任务，迟到的 transport/channel 立即释放 |
| 主机信任 | 保留 host-key 指纹检查与变更拒绝；临时首次信任策略需与无持久化要求一起实现。必要的指纹/信任选择通过专用安全契约提供，摘要不显示真实端点或认证字段 |
| 认证成功 | 删除不再需要的初始认证副本；保留 transport 所需内存资源；不留再次登录凭证缓存 |
| 关闭标签页 | 清除该目标、句柄、输出缓冲、输入历史与所属 transport/channel |
| 关闭工作区 | 取消/释放该 workspace_id 的请求与目标；其它工作区不受影响 |
| 未归属请求/关闭应用 | 未归属请求按超时或显式取消清除；退出清除所有请求/目标，进程重启不恢复 |
| 断线、授权拒绝、过期 | 进入终态并清理；提示从堡垒机重新发起。无自动重连、凭证复用或静默重试 |
| 代理/辅助进程退出 | 连接失败或断开按终态处理；不推断远端操作完成；未明确归属前不终止 AccessClient 的进程 |

认证材料应使用可清理的 Rust secret 类型，限制复制，并在消费/取消/到期时释放和清零其可控缓冲。
不得承诺能清除 OS 原始 argv、第三方库副本或 AccessClient 自身的记录。
如果 AccessClient 依赖末端进程持续存活，单实例第二进程立即退出可能破坏代理；
该行为必须在 A1 补证后决定是否需要启动器，不能仅靠内存队列认定兼容。

## 持久化、输出和授权边界

临时目标禁止创建普通保存连接、SQLite session/history 行、keychain 记录、导出项、
Cloud Sync mutation/outbox、可恢复活动详情或 Task/Flow 输入。
终端可见输出与本标签页交互输入只保留在有界内存，关闭即清除；
临时标签不提供日志导出、持久命令历史或跨会话恢复。不得自动生成活动记录以保存端点/认证绑定。
主机信任仍校验指纹；临时端点的信任关联应保留在临时目标内存中，
需单独审查现有 host-key store 的持久化入口，不能通过取消检查来避免写入。

不预先 test connection，不登录两次，不发送 OS probe，不探测额外 exec/SFTP 通道。
正式连接只建立用户请求的终端通道。任意后续能力均需 A4 独立证据与后续授权。

## 已有代码的实现注意点

基线为 `0dc4460d1619c9d85c360006cf5d4103a93dc759`：

- [`unfour-app/src/lib.rs`](../../crates/unfour-app/src/lib.rs) 的 single-instance 回调忽略 argv；
  A2 必须同时覆盖初始入口、转发和 UI 未就绪，不仅监听 UI 事件。
- [`main.rs`](../../apps/desktop/src-tauri/src/main.rs) 优先调用 build metadata CLI；
  [`update.rs`](../../apps/desktop/src-tauri/src/update.rs) 的 `--write-build-metadata` 行为需保留。
- [`desktop lib.rs`](../../apps/desktop/src-tauri/src/lib.rs) 与
  [`account.rs`](../../apps/desktop/src-tauri/src/account.rs) 的 OAuth deep-link 路径需保留；
  不能把所有 URI 都送入 SSH，也不能重注册 AccessClient 协议。
- [`ssh/session.rs`](../../crates/ssh-engine/src/ssh/session.rs) 包含保存 session 与 flush history 的路径；
  [`native_supervisor.rs`](../../crates/ssh-engine/src/ssh/native_supervisor.rs) 包含自动重连/输出处理。
  A3 不能把临时连接直接套入普通 session 的持久化与重连策略。
- [`native_auth.rs`](../../crates/ssh-engine/src/ssh/native_auth.rs)、
  [`native_transport.rs`](../../crates/ssh-engine/src/ssh/native_transport.rs)、
  [`host_key.rs`](../../crates/ssh-engine/src/host_key.rs) 是后续复用检查入口，
  本轮未验证其临时内存认证/信任路径。基线 `HostKeyStore` 首次连接采用 TOFU 并写 SQLite，
  `dial_native_connection` 直接构造该持久 store；因此不能宣称已有临时信任支持。
  临时路径必须保留匹配/变更检查，同时避免写入授权端点关联；代理场景下校验的主机身份也需补证。

## A2/A3 协作与验证门槛

A1 最小末端参数与认证/进程约束明确后，A2 和 A3 可并行实现上述边界，
各自补充 parser/dispatch 与 lifecycle/persistence 测试；没有 A2 → A3 完整阻塞边。
若 A1 未合并，后续分支基于 A1 分支，PR base 指向 A1 并说明依赖；不自行合并。

A2 必测：真实参数格式的合成回归、畸形/超长/额外开关拒绝、冷/热/延迟 UI、
连续启动不同 ID、重复 claim、队列满/超时、凭证安全错误、OAuth 和 metadata CLI 回归。
A3 必测：工作区隔离、取消/关闭竞争、认证/通道/信任等待超时、无重连与无探测、
host-key 未知/变更、安全摘要、数据库/keychain/export/sync/activity/history 零写入边界。
以上尚未运行，不能由 A1 证据工具测试替代。
