# 堡垒机外部连接：A1 内部边界草案

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
