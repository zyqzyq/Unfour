# Unfour 架构与后续模块评审

> **2026-11-18 归档说明**　以下结论和测试数字记录于 11 月 13 日。随后已实现 SSH Task 派发后结果未知的处理，以及在已登录堡垒机终端上运行 Task 和 API curl。Flow 接入临时堡垒机目标已暂缓。后续状态见[结果处理提交](https://github.com/zyqzyq/Unfour/commit/a6ed0d1ff172f928ad6ecee112d42e6a7ce31291)、[终端复用提交](https://github.com/zyqzyq/Unfour/commit/d9001d20584a84d71d767f008e7b4d84c5862660)和[现场交接记录](../../testing/bastion-offline-handoff.md)。

评审基线为 2026-11-13 的本地 `main`，提交 `49f96a43f6cba2aaf52a9f44135d72b9e4bbe880`。近期需求以 [GitHub 总任务 #3](https://github.com/zyqzyq/Unfour/issues/3) 和[现场验收 #7](https://github.com/zyqzyq/Unfour/issues/7) 为准。数据库结果交互、插件、敏感数据同步和系统签名在[路线图](../../roadmap.md)中仍属候选方向。

**建议保留按功能划分的前端包、Rust CommandBus、能力引擎和本地优先的事务模型。** 下一阶段的设计变更集中在 SSH Task 如何取得临时目标，以及远端命令结果如何传回 Flow。第二阶段开工方案的独立评审结论为 **Request changes**。第一阶段可按 #7 继续现场验收，数据库已有结果集的交互可独立开发。

## 当前结构能承接什么

| 关注点 | 本次观察 | 对后续开发的含义 |
| --- | --- | --- |
| 功能包边界 | 对 `packages/` 下 10 个包的 583 份 TS/TSX 文件做静态导入扫描，共有 25 条不同的跨包依赖边。所查禁止方向命中 0 条。Flow 前端只引用 command-client 和 ui 两个仓库内包。 | 内置功能继续由独立功能包承载，壳层负责组装。现有 CI 跑 lint、构建和测试；建议在接下一个内置模块时把禁止的依赖方向也加入 CI，包含相对路径跨包导入。[包边界](../../architecture/package-boundaries.md) · [CI](../../../.github/workflows/ci.yml) · [ESLint 配置](../../../eslint.config.mjs) |
| 命令与编排 | 用保存 Flow 走查 Desktop 和 MCP，两个适配器均委托给同一 `CommandBus::save_flow`。运行时，Flow 引擎负责验证和串行调度，SSH 动作由 CommandBus 委托给 SSH Task。 | 增加临时目标可复用原有调度和取消路径。SSH 连接生命周期由 SSH 引擎负责。[Desktop 适配器](../../../crates/unfour-app/src/commands/flow.rs) · [MCP 适配器](../../../crates/unfour-mcp/src/command_bus_adapter/implementation.rs) · [Flow 执行](../../../crates/unfour-command-bus/src/flow_commands.rs) · [引擎依赖](../../../crates/flow-engine/Cargo.toml) |
| 工作区与同步 | 保存 Flow 定义时，业务写入和同步待发记录共用 SQLite 事务。对已绑定工作区，hook 解析唯一同步归属后登记变更，网络开关控制是否唤醒发送。Desktop 和默认 MCP 进程可打开同一本地数据库，各自拥有内存中的服务实例。 | 同步定义可继续用现有机制。临时连接和认证状态归创建它的进程；同一个工作区 ID 不能让另一个进程拿到该连接。[事务](../../../crates/unfour-command-bus/src/transaction.rs) · [同步待发记录](../../../crates/unfour-cloud-sync/src/repository/outbox.rs) · [MCP 运行时](../../../crates/unfour-mcp/src/command_bus_adapter/unified_runtime.rs) |

## 第二阶段开工前的两项变更

### 1. 运行时显式绑定临时目标

今天的 SSH Task 会从显式连接、默认连接、上次使用的连接依次选择，并把选中的连接记入本地绑定和运行历史。Flow 保存 SSH 步骤要求连接 ID，运行前查找已保存连接。Workspace 导入也要求该 ID 指向可重映射的 SSH 连接。如果简单地把“临时目标失效”传成空值，后续实现可能会在另一台已保存主机上执行。[Task 目标解析及记录](../../../crates/ssh-engine/src/task/mod.rs) · [Flow 预检和执行](../../../crates/unfour-command-bus/src/flow_commands.rs) · [导入重映射](../../../crates/unfour-command-bus/src/workspace_bundle/remap.rs)

建议让可保存的 SSH 步骤明确区分“使用已保存连接”和“运行时选择临时目标”。后者只保存选择方式，实际授权只在所属桌面进程的 Rust 内存中按工作区和本次运行绑定。目标过期、关闭、属于其他工作区或运行时不可用时，在派发前停止；旧定义继续使用原有保存连接。只引入桌面临时目标的首版符合当前进程边界。以后若确有 MCP 发起此类运行的场景，再设计桌面代理的授权和生命周期。[临时会话归属](../../../crates/ssh-engine/src/ssh/temporary.rs) · [运行历史序列化](../../../crates/flow-engine/src/storage.rs) · [MCP 能力边界](../../mcp/overview.md)

**验收例**：给 Task 配一台保存的默认主机，再选一个已过期的临时目标。两台主机均应收到零次执行请求。有效目标只在所属工作区运行；历史、导出包与云端定义均不含实际句柄或票据。新定义须验证 Workspace 导入及云同步读者兼容，旧定义经过导入、导出和运行后仍使用原连接。[定义同步协议](../../../crates/unfour-cloud-sync/src/flow_definition.rs) · [Workspace 导入回归](../../../crates/unfour-command-bus/src/workspace_bundle/tests.rs)

### 2. 把派发后的“结果未知”传回 Task 和 Flow

命令可能已修改远端状态，随后通道关闭，客户端没有收到退出码。当前 SSH 驱动将这种情况归为普通步骤失败；如开启 `continueOnError`，执行器仍可继续，最后整个 Task 可返回成功。若 Task 返回普通失败，Flow 将其映射为 `FLOW_SSH_FAILED`；此处只有已取消的子 Task 映射到 `FLOW_OUTCOME_UNCERTAIN`。这与 #7 对断链后未知结果的验收要求有直接差距。[退出状态](../../../crates/ssh-engine/src/task/command_step.rs) · [步骤续跑](../../../crates/ssh-engine/src/task/runner.rs) · [Flow 结果映射](../../../crates/unfour-command-bus/src/flow_commands.rs)

建议从 SSH 驱动开始区分三种结果：未派发、已取得确定结果、已派发但结果未知。最后一种应停止有副作用的后续步骤，将不确定性传到 Task 历史与 Flow；操作人先核对远端状态，再决定是否重跑。已确认的非零退出码仍可按既有步骤策略处理。这项改动可先用隔离测试实现，与客户现场选用哪种连接方式无关。

**验收例**：在测试端记录一次副作用后、返回退出码前断开通道。即使打开 `continueOnError`，第二个步骤也应执行零次，Task 与 Flow 均应显示结果未知。用已确认的成功、非零退出和派发前失败作对照。

## 连接归属和部署顺序

Windows 打包记录已经验证两次独立的临时交互终端。真实网页到 Unfour 的完整链路、重新登录、同一连接的并发与顺序通道、原生 exec 的退出码、SFTP 上传完整性，仍需分别取证。临时终端建立连接后会释放原始启动请求；普通 Task 则在结束时断开自己建立的连接。[打包证据](../../testing/bastion-xshell-url-a2-2026-11-11.md) · [启动请求生命周期](../../../crates/ssh-engine/src/ssh/temporary/native.rs) · [Task 连接清理](../../../crates/ssh-engine/src/task/mod.rs) · [现场验收 #7](https://github.com/zyqzyq/Unfour/issues/7)

| #7 的现场结论 | 第二阶段设计决定 |
| --- | --- |
| 允许重新登录，能按需取得有效授权，原生 exec 和退出码可靠 | 优先沿用每次 Task 自建、自行释放连接的现有边界。 |
| 只允许使用已认证连接，且证实可在该连接开所需通道 | 临时会话负责连接寿命，Task 借用一次通道。明确终端关闭或授权到期时，谁终止通道、如何记录在途结果；验证结束 Task 不会断开仍在用的终端。 |
| 只验证了交互终端，或退出状态仍不可知 | 交付已验证的终端范围，继续保留自动化执行的前置条件。文件传输按独立的 SFTP 验收结果决定。 |

**K3s 离线部署先作为串行 Task/Flow 的端到端验收场景。** 固定一个系统、架构、K3s 版本和离线材料清单，验收校验和、安装后的实际状态、无状态样例健康检查和再次运行。首次部署中断后先检查已完成步骤。现有 Flow 拒绝把 SSH 动作用作通用轮询探针；只能通过 SSH 检查就绪时，可在一个明确的检查 Task 内设定超时。此场景尚未显示需要独立部署引擎、通用并行或自动恢复。[总计划 #3](https://github.com/zyqzyq/Unfour/issues/3) · [Flow 验证](../../../crates/flow-engine/src/validation.rs)

## 其他路线图候选

| 候选能力 | 现有架构能支持的范围 | 需要另作设计的触发条件 |
| --- | --- | --- |
| 更丰富的数据库结果交互 | 查询结果和表浏览复用同一网格。已返回行的交互可留在 Database 包，表浏览已有服务端排序、过滤路径。 | 若用户要对任意 SQL 的完整结果翻页，需定义结果游标或快照、取消和释放。带 `RETURNING` 的写语句可在只显示一行时完成多行写入，不能靠重跑原 SQL 翻页。[查询结果](../../../packages/database/src/components/QueryResultPanel.tsx) · [表浏览](../../../packages/database/src/components/TableDataTab.tsx) · [行数限制测试](../../../crates/database-engine/src/database_tests/result_cap.rs) |
| 插件扩展 | 内置功能可继续接入已有设置、命令和壳层插槽。工作区页签类型当前枚举四种内置模块。 | 首个需要独立安装或升级的外部模块出现时，明确其权限、版本兼容、页签恢复与失效行为，再决定公共接口及隔离方式。[现有插槽](../../../packages/app-shell/src/extensions.ts) · [桌面调用方](../../../apps/desktop/src/App.tsx) · [页签契约](../../../packages/command-client/src/types/workspace.ts) |
| 敏感数据或 Flow 运行历史同步 | 现有协议传输定义，运行历史有独立的本地生命周期。当前唯一归属和事务待发记录机制可继续承载已支持的实体。 | 先确定跨设备需要的是运行摘要、原始输出还是凭据，各自明确可见范围、密钥归属（若适用）、保留和删除规则。通过产品决策后再设计版本化的数据契约。[定义协议](../../../crates/unfour-cloud-sync/src/flow_definition.rs) · [历史隔离测试](../../../crates/unfour-cloud-sync/tests/flow/lifecycle.rs) |
| 系统签名与 macOS 公证 | 可沿发布流水线独立推进。 | 以候选安装包验证对应平台的系统信任链，分别记录更新包签名和操作系统签名结果。[签名说明](../../release/signing.md) |

## 依据与复核

本次直接采用两份 skill 原文核对代码现状、比较方案；由未参与方案撰写的只读审阅者按第三份 skill 沿 Task → Flow 路径复核。评审抽查了关键路径，静态导入扫描不能证明包内所有职责和运行时行为正确。[现状分析](https://github.com/magnus919/agent-skills/blob/main/software-architecture-analysis/SKILL.md) · [架构取舍](https://github.com/magnus919/agent-skills/blob/main/software-architecture/SKILL.md) · [独立方案评审](https://github.com/owainlewis/blueprint/blob/main/skills/architecture-review/SKILL.md)

| 本次执行 | 结果 |
| --- | --- |
| SSH 原生特性下的 `command_step::tests` 与 `runner::tests` | **10/10 通过**。覆盖退出状态分类、步骤顺序、取消及显式允许失败后继续。前述断链场景的预期行为是建议新增的验收。 |
| Cloud Sync 的 `ownership` 和 `flow` 集成测试 | **26/26 通过**。覆盖本地事务归属、离线变更、冲突及历史隔离。 |
| 前端 `QueryResultPanel`、`TableDataGrid`、`result-row-limit`、桌面 `App` 四个测试文件 | **14/14 通过**。覆盖现有结果展示与桌面组合。 |
| 迁移静态检查 | **38 个迁移文件通过**。 |

现场堡垒机、真实云服务、多设备交互、离线 K3s 部署与发布签名均未在本轮执行。相关结论以已查代码、本地定向测试和所引历史验证记录为限。
