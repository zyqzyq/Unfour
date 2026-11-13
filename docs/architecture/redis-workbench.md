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

逻辑库归属于一条连接。首版每个操作建立短生命周期连接，按其固定配置认证、选择逻辑库并在操作后释放。不同工作区或不同逻辑库的读取不会共享 SELECT 状态。如果后续性能数据表明 TLS 握手成为瓶颈，再按工作区、连接 ID、修订和逻辑库复用连接。首版按用户配置的单实例地址读取，Cluster、Sentinel、共享 SSH 隧道和修改缓存数据留待对应的协议及权限设计。

SCAN 的 COUNT 是工作量提示。键集合在扫描中可能变化，UI 不能据此推导全库总数或稳定页码。PTTL 为 -1 表示永久，为 -2 表示键已不存在。键在读取元信息与内容之间可能过期或改变类型，刷新后以新结果为准。[SCAN](https://redis.io/docs/latest/commands/scan/) · [PTTL](https://redis.io/docs/latest/commands/pttl/) · [GETRANGE](https://redis.io/docs/latest/commands/getrange/)

## 边界和契约

桌面调用路径为 Database 中的 Redis 组件 → command-client → Tauri → CommandBus → redis-engine → Redis。Database 功能包拥有连接树和对象工作区，可以将 Redis 组件放在本包独立目录。SQL 执行、SQL 驱动枚举、app-shell 导航均保留原职责。Redis 驱动使用类型化操作，编码命令参数为独立 RESP 数组成员；用户输入不能成为命令名。连接、读写和 RESP 分配都需要预算，超出字节上限的操作返回稳定错误码，释放连接。浏览器开发模式可以模拟连接列表，真实网络读取需要桌面运行时。

| 桌面命令 | 输入及输出 |
| --- | --- |
| redis_connections_list | workspaceId → 已保存的脱敏连接；仅暴露 hasPassword。 |
| redis_connection_save / redis_connection_delete | 工作区、草稿或资源 ID、编辑修订；独立 secret 参数区分保留、轮换和清除。 |
| redis_connection_test_input | 测试当前草稿，无需预先保存。 |
| redis_key_inspect | 工作区、连接、base64 编码的 key 和可选字段 → 存在性、类型、毫秒 TTL、长度、可选预览和截断标识。 |
| redis_keys_scan | 工作区、连接、模式、继续标记 → 可逆编码的键及下个标记。`"0"` 表示起始或结束。其余标记绑定工作区、连接 ID、修订号、逻辑库和匹配模式，由界面原样透传。 |

读取操作不持久化 Redis 的键名、字段、值、命令参数或扫描记录。错误和活动事件只接受稳定状态和非敏感元信息。密码经凭据暂存、业务提交、回收流程进入系统凭据存储。检查连接草稿时，只有原端点才可复用已存密码。

驱动直接使用工作区锁定的 Tokio、tokio-rustls、rustls-platform-verifier 与 base64。引擎仅解码这些只读操作所需的 RESP2 回复形状，在分配 bulk string 之前检查声明长度。单次网络操作总限时 8 秒，String 和 Hash 字段的值预览预算为 16 KiB，SCAN 单批原始键名预算为 256 KiB。大 String 可读取前缀，大 Hash 字段返回大小超限。TLS 使用系统信任链和真实主机名验证。

## 持久化和交换

SQLite 的 connections 公共表新增 redis 子类型。redis_connections 保存 workspace_id、connection_id、ACL 用户名、逻辑库和 TLS 配置，通过同工作区的复合外键关联公共记录。迁移须保留既有 SQL/SSH 连接及所有子表、索引和触发器。工作区移除、单连接删除与远端删除都处理 tombstone 和设备凭据生命周期。

连接定义沿用现有 Cloud Sync 的 connection 信封。Redis 子类型包含非敏感配置，凭据引用及运行数据留在设备。绑定工作区的本地业务操作和唯一归属的 outbox 意图一起提交，离线、退出登录、暂停均不改变该约束。远端应用遵守 workspace/类型归属、幂等重放；远端改变主机、端口、用户名或 TLS 时解绑本地认证材料。旧端使用现有未来子类型兼容流程。真实云服务接收 Redis 子类型须单独验证。

含 Redis 连接的工作区文件使用 V3。没有 Redis 连接的工作区继续使用原有格式，新客户端可读取旧文件。导入为独立副本，重新生成连接 ID。普通导出不含密码；选择包含凭据的密码加密备份才可还原密码，恢复时生成新凭据引用。未知子类型必须明确拒绝或按同步协议延迟处理。

## Flow 与 MCP

Flow 的 Redis 步骤使用连接 ID 作为 resourceId，定义接受 key、可选 field 和可选 equals。无 equals 时检查键或指定字段存在；有 equals 时对完整字符串或 Hash 字段作字节比较。输出包含 exists、keyType、pttlMs 和 matched，供 Condition 或 Wait Until 使用。截断的内容不能判为相等。从桌面详情创建 Flow 时，生成未保存的草稿，实际键预填在声明为 secret 的运行时输入中。运行历史隐藏 Redis 步骤的解析参数。用户手工填写到定义里的键模板或文字常量，会随定义导出或同步。

MCP 客户端通过 unfour.connection.list 发现 Redis 连接，再使用 unfour.redis.check_key 执行同一项精确检查。工具绑定调用所授权的工作区，仅返回元数据和比较结果。整条 Flow 仍适用原有运行权限。

## 验收

- 将有现存 SQL/SSH 连接、凭据引用及关联业务记录的 SQLite 库升级，核对行数、索引和外键。
- 无认证、ACL、TLS 与错误凭据分别测试；目标变化时不得发送原密码。用两个同名 key 的逻辑库验证隔离。
- 测试缺失键、空字符串、二进制键、过期、类型变化、超大回复、零结果但游标未结束和网络超时。
- 在同一个 Database 工作区打开 SQL 草稿和 Redis，来回切换，再切工作区，检查草稿与 Redis 读取的归属。
- 测试旧格式导入、含 Redis 的普通导入导出和加密凭据备份。云端验证区分本地事务与真实双设备同步。
