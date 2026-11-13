# Redis 接入设计

## 用户入口

保留 API、SSH、Database、Flow 四个一级入口。Database 侧栏的新增按钮打开连接表单，在驱动选项中选择 Redis。保存后 SQL 和 Redis 连接出现在同一列表，按连接类型显示图标。打开 SQL 连接进入现有查询与表视图，打开 Redis 进入 Key 浏览器；切换两类对象时保留 SQL 草稿。工作区切换清除上一工作区的缓存内容和在途结果。所有新界面文案使用共享中英文词条。

DataGrip 的 Database Explorer 使用共用的数据源入口，Redis 连接另有键过滤条件。[DataGrip](https://www.jetbrains.com/help/datagrip/redis.html)

Redis Insight 按数据类型呈现内容，并提供 JSON 和十六进制格式。Unfour 采用精确键查询作为主操作，便于开发者直接检查从 API 或数据库得到的业务 ID。[Redis Insight](https://redis.io/docs/latest/develop/tools/insight/)

## 首个交付范围

| 操作 | 设计和可观察结果 |
| --- | --- |
| 新建、测试、编辑、删除连接 | 独立 Redis 单实例，支持 TCP、校验服务名和系统信任链的 TLS、可选 ACL 用户名与密码、逻辑库编号。密码只在明确填写时更新。使用配置修订检测并发编辑。改变认证目标时重新确认凭据。 |
| 精确查找 | 输入完整 key 后直接读取，无需 SCAN 权限。显示来源连接、逻辑库、类型、TTL 和可读取的内容。不存在、已过期、无过期时间、权限不足、数据超过预览预算分别呈现。 |
| 按模式浏览 | 显式启动 SCAN，每次只向服务端请求一个批次，允许停止继续加载。返回游标为零才结束。换连接、修订或模式时作废先前浏览状态。浏览结果可能包含重复键，空批次可以继续扫描。 |
| 类型化详情 | String 用 GETRANGE 读取有限字节；完整 UTF-8 JSON 可格式化，其他字节保留可逆表示。Hash 展示字段数并支持查询指定字段。List、Set、ZSet、Stream 首先显示元素数。未知类型保留 Redis 返回的类型标识。 |

逻辑库归属于一条连接。每个操作建立短生命周期连接，按其固定配置认证、选择逻辑库并在操作后释放。当前支持独立 Redis 单实例，可直连、复用同工作区已保存的 SSH 连接，或选用已连接的堡垒机临时会话。临时会话覆盖仅对当前窗口生效，使用该路由时可单独输入 Redis 目标密码。经 SSH 时由跳板解析 Redis 内网域名，Redis TLS 仍校验该域名。变更跳板地址、账户或受信指纹后，需重新核对 Redis 连接及凭据。工作区导入会重映射跳板 ID，缺失的跳板不会触发直连。Cluster 和 Sentinel 另行设计。[共享 SSH 隧道设计](shared-ssh-tunnels.md)

SCAN 的 COUNT 是工作量提示。键集合在扫描中可能变化，UI 不能据此推导全库总数或稳定页码。PTTL 为 -1 表示永久，为 -2 表示键已不存在。键在读取元信息与内容之间可能过期或改变类型，刷新后以新结果为准。[SCAN](https://redis.io/docs/latest/commands/scan/) · [PTTL](https://redis.io/docs/latest/commands/pttl/) · [GETRANGE](https://redis.io/docs/latest/commands/getrange/)

## 边界和契约

桌面调用路径为 Database 中的 Redis 组件 → command-client → Tauri → CommandBus → redis-engine → Redis。Database 功能包拥有连接树和对象工作区，可以将 Redis 组件放在本包独立目录。SQL 执行、SQL 驱动枚举、app-shell 导航均保留原职责。Redis 驱动使用类型化操作，编码命令参数为独立 RESP 数组成员；用户输入不能成为命令名。连接、读写和 RESP 分配都需要预算，超出字节上限的操作返回稳定错误码，释放连接。浏览器开发模式可以模拟连接列表，真实网络读取需要桌面运行时。

| 桌面命令 | 输入及输出 |
| --- | --- |
| redis_connections_list | workspaceId → 已保存的连接、是否配置密码及本机写入许可。 |
| redis_connection_save / redis_connection_delete | 工作区、草稿或资源 ID、编辑修订；独立 secret 参数区分保留、轮换和清除。 |
| redis_connection_test_input | 测试当前草稿，无需预先保存。 |
| redis_key_inspect | 工作区、连接、base64 编码的 key 和可选字段 → 存在性、类型、毫秒 TTL、长度、可选预览和截断标识。 |
| redis_keys_scan | 工作区、连接、模式、继续标记 → 可逆编码的键及下个标记。`"0"` 表示起始或结束。其余标记绑定工作区、连接 ID、修订号、逻辑库和匹配模式，由界面原样透传。 |
| redis_collection_browse | 工作区、连接及修订、精确键、类型、模式或排序方向、继续标记 → 成员和下个标记。List 和 ZSet 支持按索引或排名开始。 |
| redis_key_mutate | 工作区、连接及修订、精确键、类型化变更、显式确认 → 执行结果。编辑 String 或 Hash 字段时须传入原始完整字节。 |

读取操作不持久化 Redis 的键名、字段、值、命令参数或扫描记录。错误和活动事件只接受稳定状态和非敏感元信息。密码经凭据暂存、业务提交、回收流程进入系统凭据存储。检查连接草稿时，只有原端点才可复用已存密码。

驱动直接使用工作区锁定的 Tokio、tokio-rustls、rustls-platform-verifier 与 base64。引擎仅解码支持操作所需的 RESP2 回复形状，在分配 bulk string 之前检查声明长度。单次网络操作总限时 8 秒，String 和 Hash 字段的完整值编辑预算为 16 KiB，SCAN 单批原始键名预算为 256 KiB。大 String 可读取前缀，超限 Hash 字段的精确查询会报告大小超限。集合浏览另设单成员 64 KiB、单批数据 256 KiB 预算。TLS 使用系统信任链和真实主机名验证。

## 集合浏览和桌面编辑

DbGate 在同一数据源工作台中提供类型化编辑和保存前的命令预览，Redis Insight 依类型展示成员。Unfour 沿用现有 Database 标签页，在键详情中展开成员；写操作先给出连接、逻辑库、精确键、变更内容与过期策略，点击确认后才提交。[DbGate Redis GUI](https://www.dbgate.io/features/redis-gui/) · [Redis Insight](https://redis.io/docs/latest/develop/tools/insight/)

| 操作 | 交互与执行语义 |
| --- | --- |
| Hash、Set 浏览 | 按 Redis glob 过滤字段或成员并继续扫描。Hash 同时显示字段和值，可选中某个字段查看或编辑。继续标记绑定连接修订、键、类型、筛选和方向。空批次仍可继续，直到游标归零。 |
| List、ZSet 浏览 | 通过索引或排名分段读取。List 展示真实索引；ZSet 展示排序方向下的排名、成员和 score，支持升降序。读取预算约束元素数和协议字节数。并发插入、删除或调整分数时，后续各页代表当次读取时的集合。 |
| String | 新建使用仅当键不存在的 SET，并可同时设置正 TTL。编辑从完整原始值开始，提交时比较类型和旧字节，更新保留原 TTL。截断预览不能提交为整键值。 |
| Hash 字段 | 新建 Hash 同时写入第一个字段。增字段须确认原字段不存在；改、删只作用于一个精确字段，提交时比对旧值。删除最后一个字段会一并移除键。编辑保留整键 TTL；Redis 8+ 还保留字段 TTL，7.4 上遇到有独立 TTL 的字段拒绝覆盖。 |
| TTL 与删除 | 针对选中的单键设置正 TTL，或单独选择移除过期时间。删除键用 UNLINK；用户看到键名、类型、连接和逻辑库。过期时间与删除操作针对确认的键名和类型，不比较整个集合的每个值。 |

现有与新增连接默认只读。本机允许写入的设置保存在 Redis 连接附表。同步、普通导入和加密备份恢复都不会转移写入许可。加密备份可以单独还原凭据。远端改变端点、登录身份、TLS 或逻辑库时，撤销原目标的写许可。手工写入必须经过 Tauri → CommandBus 的 workspace/连接修订/本机许可/显式确认检查。Redis ACL 仍决定服务端能否执行。Flow 和 MCP 继续只读。活动只记操作种类、连接 ID 和结果。

写入以 Redis 6.0.9+ 为基线，通过 INFO server 确认版本。每次写入在同一连接中 WATCH 并检查键/字段状态，再在事务内执行固定的单项变更。提交前检测到冲突时提示重读；EXEC 已派发而回复丢失时显示结果未知，用户须核对实际状态。客户端不自动重发。编辑时发现已过期的键不会重建。WATCH 捕获检查开始后的并发修改；初始编辑的旧值比较不会检测到历史上改过又恢复为相同字节的情况。[Redis 事务](https://redis.io/docs/latest/develop/using-commands/transactions/) · [SET](https://redis.io/docs/latest/commands/set/) · [HSETEX](https://redis.io/docs/latest/commands/hsetex/) · [EXPIRE](https://redis.io/docs/latest/commands/expire/) · [UNLINK](https://redis.io/docs/latest/commands/unlink/)

## 持久化和交换

SQLite 的 connections 公共表新增 redis 子类型。redis_connections 保存 workspace_id、connection_id、ACL 用户名、逻辑库和 TLS 配置，通过同工作区的复合外键关联公共记录。迁移须保留既有 SQL/SSH 连接及所有子表、索引和触发器。工作区移除、单连接删除与远端删除都处理 tombstone 和设备凭据生命周期。

连接定义沿用现有 Cloud Sync 的 connection 信封。Redis 子类型包含非敏感配置，凭据引用及运行数据留在设备。绑定工作区的本地业务操作和唯一归属的 outbox 意图一起提交，离线、退出登录、暂停均不改变该约束。远端应用遵守 workspace/类型归属、幂等重放；远端改变主机、端口、用户名或 TLS 时解绑本地认证材料。旧端使用现有未来子类型兼容流程。真实云服务接收 Redis 子类型须单独验证。

含 Redis 连接的工作区文件使用 V3。没有 Redis 连接的工作区继续使用原有格式，新客户端可读取旧文件。导入为独立副本，重新生成连接 ID。普通导出不含密码；选择包含凭据的密码加密备份才可还原密码，恢复时生成新凭据引用。未知子类型必须明确拒绝或按同步协议延迟处理。

桌面测试和保存 SSH 路由时，会校验表单中显示的跳板修订。跳板目标已更改时，刷新选择后才可使用该路由。加密备份恢复后仍需复核导出时已经变更的跳板，重新确认凭据后才可使用。工作区有 Redis 引用已删除的 SSH 连接时，需先修复连接定义再导出。

## Flow 与 MCP

Flow 的 Redis 步骤使用连接 ID 作为 resourceId，定义接受 key、可选 field 和可选 equals。无 equals 时检查键或指定字段存在；有 equals 时对完整字符串或 Hash 字段作字节比较。输出包含 exists、keyType、pttlMs 和 matched，供 Condition 或 Wait Until 使用。截断的内容不能判为相等。从桌面详情创建 Flow 时，生成未保存的草稿，实际键预填在声明为 secret 的运行时输入中。运行历史隐藏 Redis 步骤的解析参数。用户手工填写到定义里的键模板或文字常量，会随定义导出或同步。

MCP 客户端通过 unfour.connection.list 发现 Redis 连接，再使用 unfour.redis.check_key 执行同一项精确检查。工具绑定调用所授权的工作区，仅返回元数据和比较结果。整条 Flow 仍适用原有运行权限。

## 验收

- 将有现存 SQL/SSH 连接、凭据引用及关联业务记录的 SQLite 库升级，核对行数、索引和外键。
- 无认证、ACL、TLS 与错误凭据分别测试；目标变化时不得发送原密码。用两个同名 key 的逻辑库验证隔离。
- 测试缺失键、空字符串、二进制键、过期、类型变化、超大回复、零结果但游标未结束和网络超时。
- 在同一个 Database 工作区打开 SQL 草稿和 Redis，来回切换，再切工作区，检查草稿与 Redis 读取的归属。
- 测试旧格式导入、含 Redis 的普通导入导出和加密凭据备份。云端验证区分本地事务与真实双设备同步。
