# 仓库可维护性评估与 Database 重构验证（2026-10-12）

## 基线与评估方法

- 分支：直接在 `main` 修改；开始时工作区干净。
- 已执行 `git fetch origin main`；评估基线 HEAD 与 origin/main 均为
  `b5e11dced4e051cf0eee1ad0c4e1041920b68375`。
- 全仓执行 ESLint 与 `check:large-files`，检查 package/Cargo 依赖声明、前端包导入，
  结合最近 **100 个仓库提交**的文件改动次数、现有测试和热点代码阅读排序。
  基线跟踪文件包含 300 个 `.ts`、302 个 `.tsx`、461 个 `.rs`。
- 这是全仓静态扫描加重点路径审阅，不是每行代码的安全审计，也不是覆盖率百分比审计。
  Rust/MCP/Cloud Sync 结论来自结构和代码阅读，本轮没有运行它们的原生集成测试。
- 不改 ESLint、超长文件阈值/排除项/baseline、AGENTS.md、依赖、公共导出和持久化格式。

## 架构判断与优先级

整体模块方向合理：feature 使用 command-client/UI/workspace-core；app-shell 组合模块和
sidebar slots；workspace-local 保留本地生命周期；Rust 执行仍经 CommandBus。
本轮检查的依赖声明、包导入和抽查调用路径未发现明确的 feature 反向依赖或 UI 包吸收业务。
主要问题在包内职责集中和回归保护不足，不需要重建包结构。

下表优先级表示后续工程投入顺序，**不是**超长文件脚本的 P0/P1 长度分类。
改动次数统一为基线之前最近 100 个仓库提交，不是文件自己的最近 100 次修改。

| 优先级 | 问题与代码证据 | 覆盖、风险及决定 |
| --- | --- | --- |
| P1，本轮 | `packages/command-client/src/tauri/browser-mocks/database.ts`：18 个命令分支，单函数复杂度 123、有效函数长度 412；连接写入、Schema fixture、SQL 校验、分页、Saved SQL 全部集中。最近 100 提交修改 5 次。 | 原有 `tauri.test.ts` 有 MySQL 与 Saved SQL 集成用例，但缺少完整的路由/空返回/默认值保护。可用浏览器内存 fixture 独立验证，收益明确、生产执行风险低。按动作职责提取并补契约测试。 |
| P1，本轮 | `DatabaseConnectionTree.tsx` 749 物理行、664 ESLint 有效行；交互状态与目录/Schema/表/Saved SQL 构建混合，多层重复传递相同 callbacks、maps、翻译函数。最近 100 提交修改 3 次（文件完整历史 25 次）。 | 已有组件与 controller 测试；补目录加载、折叠和选择语义，再拆同步节点构建。保留组件的会话 gate、TreeView key、弹窗和事件路由。 |
| P1，下一轮 | browser mocks 的 workspace（复杂度 173）、API collections（96）、SSH（91）、API requests（87）。已按 feature 分文件，但一个 feature 内的巨大 dispatcher 仍是修改热点。 | 优先 workspace 的环境变量 CRUD：有 `workspace.test.ts`，先锁定排序、active environment、secret 掩码及清理行为。API 执行/SSH events 有异步时序，不能把 Database 的同步 Map 模式直接套过去。 |
| P1，下一轮 | `packages/flow/src/model.ts` 的 `resourceErrors` 将 API/Database/SSH 资源检查与 probe 限制放在同一回调，复杂度 33；最近 100 提交修改 9 次，FlowPage/StepEditor 同样活跃。 | 已有 model、authoringValidation、environment validation 和页面测试。可按 capability 提取校验，必须保留错误顺序、missing resource 的提前返回、环境 defaults 语义。JSON 草稿同步警告需结合 `JsonField.test.tsx` 单独处理，不能直接删除 effect。 |
| P2，专项 | Database active context（复杂度 28）、query actions、SQL runner、browse 与 workspace controller 的长函数，SSH session hooks/TerminalPane/SftpPanel。 | 存在真实编排复杂度，但取消、过期响应、工作区切换和 cleanup 互相约束；已有 hooks/model 拆分有价值。先补时序矩阵，再拆控制流，不为函数长度继续分散 refs 和 effects。 |
| P2，专项 | `crates/unfour-command-bus/src/api_commands.rs` 866 行，包含 legacy environments、collection/folder/request、layout 与执行入口；最近 100 提交修改 5 次。 | 比单纯转发更有职责混合；按命令族拆分值得做，但要保留 transaction/activity/outbox 的边界，不能抽出吞掉 policy 的通用 CRUD。 |
| P2，高风险 | MCP `command_bus_adapter.rs` 1151 行、最近 100 提交修改 10 次；`tools/database.rs` 958 行；Cloud Sync `canonical.rs` 1007 行。 | Adapter 的大量显式转发、tool schema 和 protocol allowlist 有可读性价值。canonical 同时 encode/decode 多实体，确有演进压力；先做协议 golden/round-trip/secret omission 和确认契约矩阵。不要以通用序列化替代白名单，不动 outbox owner/事务。 |
| P3，可接受 | 长测试文件、声明型协议/schema、组合页面和 TreeView。 | 88 个长文件中 49 个被脚本识别为含测试，0 Critical；例如 outbox 测试 1198 行、FlowPage 测试 1011 行。长集成场景本身不是设计缺陷。TreeView 的焦点/键盘/DnD 共用生命周期；拆散可能增加风险。 |

其他值得保留的边界：Database 的纯树分组已在 `model/database-tree.ts`，菜单在
`database-tree-menus.tsx`，schema 请求在 hooks；本次提取接续这些职责，而非另建树框架。
Database/SSH 后端连接 CRUD 看起来类似，但凭据、driver、outbox 和外部 apply 规则不同，
本轮没有证据支持引入跨 feature 通用连接服务。

## 本轮结构变化

### Database 树

- `DatabaseConnectionTree.tsx`：连接状态、根节点、选择/激活/懒加载回调、删除确认、导出弹窗。
- 新 `database-tree-content.tsx`：同步构建目录/Schema/表/视图/Saved SQL 节点，
  使用一个 `ContentContext` 传递连接、翻译、动作和本次 render 的索引。
- `createDatabaseTreeIndex` 明确 lookup/default expansion 的共同生命周期；没有缓存跨 render
  的旧对象，没有引入新的 effect、请求或全局状态。
- 移除 child builder 中 disconnected/failed 分支：原唯一调用方已经只在 connected 时调用。
- 保留 PostgreSQL/MySQL 目录、SQLite 根层级，原节点 ID/标题/icon/badge，表组自动展开、
  视图组默认折叠，catalog-scoped Saved SQL 及 SQLite 等待 Schema 的既有差异。
- 原有两处英文直写文案原样移动，本轮没有新增产品文案或翻译系统。

### Database 浏览器 mock

| 文件 | 职责 |
| --- | --- |
| `database.ts` | 显式注册 18 个命令，解码参数；通过 handler 是否存在判断 UNHANDLED，保留合法 undefined 返回。 |
| `database-connections.ts` | 连接的内存记录、默认配置、版本/时间、测试结果与既有 credential reference fixture 行为。 |
| `database-schema.ts` | 各 driver 的目录和 Schema fixture；每次调用返回新对象，避免共享常量被调用方修改。 |
| `database-saved-sql.ts` | 输入规范化/校验和工作区内的 snippet CRUD、排序。 |
| `database-queries.ts` | 查询、分页、结构、行变更结果 fixture；确认错误和标识符引用保持原样。 |

所有文件仍在 command-client 的 browser-mocks 下；不新增生产数据库执行逻辑或公共 API。
`Map` 只用于这个同步 dispatcher，没有扩张成全仓命令框架。连接配置默认值和 Saved SQL
输入校验与记录更新分离，减少修改默认值/校验规则时需要阅读的控制流。

## 前后对比

同一 ESLint 配置和相同 `pnpm run check:large-files --json --top 300` 参数：

| 指标 | 基线 | 重构后 |
| --- | ---: | ---: |
| ESLint errors | 0 | 0 |
| ESLint warnings | 41 | 38 |
| complexity warnings | 16 | 15 |
| max-lines-per-function warnings | 21 | 20 |
| max-lines warnings | 2 | 1 |
| unused-vars / set-state-in-effect | 1 / 1 | 1 / 1 |
| 超长文件总数 | 88 | 87 |
| Critical / P0 / P1 | 0 / 20 / 68 | 0 / 20 / 67 |
| blocking / grandfathered | 0 / 0 | 0 / 0 |
| 含测试的超长文件 | 49 | 49 |

变化限于树的 1 条文件长度警告，以及 Database mock 的复杂度/函数长度 2 条警告。
所有本轮修改和新增的 TS/TSX 文件均无 ESLint 警告；未修改规则或添加 suppression。

物理行数按仓库 checker 的 `countLines` 计算：

| 代码范围 | 基线 | 重构后 |
| --- | ---: | ---: |
| DatabaseConnectionTree | 749 | 296 |
| 新树内容 builder | 0 | 215 |
| 树实现合计 | 749 | 511 |
| Database mock 入口 | 453 | 62 |
| mock 连接 / Schema / Saved SQL / 查询模块 | 0 | 85 / 128 / 55 / 152 |
| mock 实现合计 | 453 | 482 |

mock 总行数略增，收益在于 412 有效行、复杂度 123 的统一函数被独立可测操作取代，
不是文件总量变小。树实现减少 238 物理行，主要消除重复的类型/参数穿透和不可达分支。
没有对其它警告做附带清理以扩大数字收益。

## 回归与验证

新增 19 个 mock 契约用例、2 个树组件用例，以及 1 个浏览器端到端用例。
先在原实现执行新增单测，再修改实现；不是只验证拆分后的代码。

| 检查 | 结果与边界 |
| --- | --- |
| 重构前：database mock tests + tauri.test + DatabaseConnectionTree.test | PASS，3 文件 / 43 测试。第一次新增树用例误写既有文案为 Loading schema…，出现 1 FAIL；改为真实 Loading… 后原实现全部通过，没有改产品文案。 |
| 重构后：`pnpm exec vitest run packages/database packages/command-client` | PASS，41 文件 / 252 测试。 |
| 最终：`pnpm test` | PASS，171 文件 / 1063 测试。未运行基线的全量 suite，不能据此声称消除了原有全仓测试失败。 |
| `pnpm exec playwright test apps/desktop/tests/smoke/database-tree.spec.ts --workers=1` | PASS，Chromium 1 测试；一次性 browser fixture 中走通连接、PostgreSQL reporting 懒加载、视图展开、表预览、删除取消/确认。查看生成截图，布局与预期一致。 |
| `pnpm run build` | PASS，TypeScript + Vite 生产构建；保留 >500 kB chunk 提示，没有修改 bundle warning 阈值。 |
| 全仓 ESLint JSON 输出 | PASS，0 errors / 38 warnings。 |
| `pnpm run check:large-files --json --top 300` | PASS，0 blocking；87 个提示。 |
| `git diff --check` | PASS。 |

新增测试保护的细节还包括：unknown/inherited 命令不误派发、void history 命令不误判未处理、
连接创建/更新顺序和时间/版本、工作区 list/delete、SQLite 忽略 secret、Saved SQL trim 和校验错误、
三种 driver 的目录/版本、PostgreSQL reporting 的独立对象、分页估计总数及引用转义、
查询/行修改确认、树的单击与双击区别、手动折叠在缓存刷新后保留、稳定节点 ID。

工具环境：初始 sandbox 中 Git FETCH_HEAD 写入与 pnpm 本机缓存访问失败；使用授权范围内的
提升权限执行后完成 fetch 和全部检查，没有因此跳过验证或修改仓库配置。

## 剩余风险与下一轮

- NOT RUN：Rust workspace tests/check、真实 SQLite/PostgreSQL/MySQL、原生 Tauri/MCP、
  Cloud Sync 服务集成。生产 Rust 和调用协议未修改；浏览器通过不代表这些路径通过。
- 浏览器 mock 的 SQL classification、MySQL schema fixture 和权限语义本来就不是生产引擎。
  保留现有行为；不能将本次拆分视为对 mock 与 Rust 完全等价的认证。
- 没有增加覆盖率百分比声明，也没有建立构建体积的前后基线；chunk 提示仍需独立分析。
- 长度下降不消除树的业务复杂性。未知 catalog、混合 Schema、workspace 切换和异步取消仍应由
  现有 hooks/controller 测试和后续原生验证共同保护。
- 下一轮首选 workspace mock 按环境/变量/生命周期拆分，同时补 route/result 契约；
  接着处理 Flow resource validation 的 capability 分工。MCP/Cloud Sync 先补协议与安全契约
  corpus，再设计小步重构，不以告警数量决定优先级。
