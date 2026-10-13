# 第二轮可维护性重构：Workspace mock 与 Flow 资源校验

## 基线与选择依据

延续 [2026-10-12 评估](architecture-health-2026-10-12.md) 的模块边界、行为兼容和验证要求。
已重新 fetch：开始时 main 与 origin/main 均为
`f4c03794f4b964e1ac913330789cdcfbe568f495`，工作区干净。
本轮继续直接修改 main，不创建分支、不修改 AGENTS.md。

本轮重新执行全仓 Lint、超长文件检查，并审阅两个优先候选及其消费者和测试。
基线为 0 ESLint errors / 38 warnings，87 个超长文件，无 Critical 或 blocking。
没有重新声称做了整仓逐行审计；本次是上一轮评估的优先事项落地。

| 优先级 | 原问题 | 本轮决定 |
| --- | --- | --- |
| P1 | workspace browser mock 单函数复杂度 173、有效函数长度 381；31 条命令混合 workspace 生命周期、布局、环境、变量 CRUD/解析。 | 按职责拆开命令操作，路由只负责查找/调用；不引入通用 CommandBus 或插件框架。 |
| P1 | 工作区变量替换、环境整体更新、环境变量替换三处重复维护 id、创建时间、revision 和排序。 | 共享变量记录构造，同时保留各操作不同的环境 metadata/revision、scope 和返回值。 |
| P1 | Flow `resourceErrors` 回调复杂度 33；资源查找、API 限制、SQL 参数、SSH 输入和 probe 限制混合。 | 保留步序与错误顺序，按 capability 分解为纯校验函数；现有 model 导出继续兼容。 |
| 后续 | API collections mock（复杂度 96）、SSH transcript（37）、异步 session hooks。 | collections 可按 folder/collection 操作继续拆；transcript 先补乱序、缺失、完成/运行中 transfer 用例；session hooks 需要专项时序测试。 |
| 保留 | MCP/Cloud Sync 协议、长集成测试及声明式 schema。 | 延续上一轮风险判断，不为行数修改安全或协议边界。 |

两个目标文件原来分别只有 436、176 物理行，说明本轮优先级并非由文件长度排序得出。

## 实际结构与保持的行为

Workspace mock 内部模块：

- `workspace.ts`：31 条命令的显式 Map 注册、UNHANDLED 路由；与基线提取出的命令集合完全相同。
- `workspace-lifecycle.ts`：workspace 创建/切换/删除、环境类型、MCP policy 字段、默认工作区及布局。
- `workspace-environments.ts`：变量环境与其变量操作、排序、激活和删除后的选择。
- `workspace-variables.ts`：工作区变量操作及模板解析。
- `workspace-variable-records.ts`：纯记录构造和原地更新，不访问 mockStore，不决定 scope、激活或环境版本。

提取前的三处批量变量构造现在共用 `replaceMockVariables`；两处 create 和两处 update
也共用记录处理。scope 筛选在各调用方完成；没有跨 feature 或生产/fixture 复用。
新函数内部保留原参数解码与默认值，不用一个统一 workspace fallback 改变命令差异。

明确保持：

- 布局和 active workspace 命令的 fallback，与变量命令空 workspaceId 的既有区别。
- 首个环境自动激活、null 取消激活、删除 active 环境后选择排序中的首个剩余环境。
- reorder 必须完整且无重复，校验失败前不写入，只有位置改变才更新 revision/time。
- 环境整体更新增加环境 revision；单独替换环境变量不增加；激活本身不增加 revision。
- 变量创建保留传入 sortOrder；批量替换按输入顺序重排，保留匹配记录的 id/createdAt 并增加 revision。
- 变量 update 原地修改，且不能由 input.id 改写目标 id；替换保持其它 workspace 记录。
- 解析器大小写敏感、仅 enabled 的环境值覆盖工作区值、缺失变量/显式错误环境继续抛原错误。
- Browser picker 的 null、export 的 `{ saved: false }` 及未知命令的 UNHANDLED 保持不变。

Flow 的 `resourceValidation.ts` 承接 Resources、resourceErrors 和 environmentInputErrors，
`model.ts` 继续 re-export 相同类型与函数，消费方无须迁移，也没有运行时循环依赖。
每种 capability 有独立校验；missing resource 仍提前返回唯一错误，非执行节点仍跳过。
保持 SQL 单语句反馈、literal limit 边界、动态参数的延迟校验、GET/HEAD probe、API script/multipart
限制、SSH 显式输入与 workspace defaults 的原语义。没有把前端校验升级为执行安全边界。

## 指标对比

同一 ESLint 配置、同一超长文件检查器；未添加 suppression、修改阈值、排除项或 baseline。

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| ESLint errors | 0 | 0 |
| ESLint warnings | 38 | 35 |
| complexity warnings | 15 | 13 |
| max-lines-per-function warnings | 20 | 19 |
| max-lines / unused-vars / set-state-in-effect | 1 / 1 / 1 | 1 / 1 / 1 |
| 超长文件 | 87 | 87 |
| Critical / P0 / P1 | 0 / 20 / 67 | 0 / 20 / 67 |
| blocking / grandfathered | 0 / 0 | 0 / 0 |

消除的是 workspace 的复杂度/函数长度两条，以及 Flow 回调的复杂度一条。
本轮所有新增或修改的 TS 文件均无 Lint 警告；所有新操作/校验函数符合原复杂度 15 的限制。

物理行数按仓库 `countLines` 统计：

| 实现 | 修改前 | 修改后 |
| --- | ---: | ---: |
| workspace 入口 | 436 | 45 |
| lifecycle / environments / variables / records | 0 | 104 / 174 / 85 / 45 |
| workspace 实现合计 | 436 | 453 |
| Flow model | 176 | 120 |
| Flow resource validation | 0 | 88 |
| Flow 实现合计 | 176 | 208 |

总实现行数增加 49 行，来自独立函数边界、注册和校验返回值；收益是修改范围和测试边界更清晰，
并集中重复记录规则，而不是代码压缩或减少超长文件数量。

## 验证证据

新增 32 项单测（workspace contract 13 项、Flow resource validation 19 项），
先在原实现执行，再做实现重构。测试使用内存 fixture，结束后恢复 mock 状态和时钟。

| 命令/检查 | 结果 |
| --- | --- |
| 重构前，workspace-contract/workspace/resourceValidation/model/flowPageValidation/actionAuthoring 六个测试文件 | PASS：6 文件、62 项测试。 |
| `pnpm exec vitest run packages/command-client packages/flow packages/workspace-environments packages/workspace-core packages/workspace-local` | PASS：34 文件、322 项测试。 |
| `pnpm test` | PASS：173 文件、1095 项测试；上一轮相同起点记录为 171 文件、1063 项。 |
| `pnpm run build` | PASS：TypeScript + Vite。仍有 >500 kB chunk 提示，没有调阈值；不宣称本轮解决 bundle 体积。 |
| `pnpm exec eslint . -f json -o <temporary-report>` | PASS：0 errors、35 warnings。 |
| `pnpm run check:large-files --json --top 300` | PASS：87 个提示、0 blocking。 |
| `git diff --check` | PASS。 |

浏览器检查命令：

```text
pnpm exec playwright test apps/desktop/tests/smoke/desktop-ux.spec.ts apps/desktop/tests/smoke/flow-authoring.spec.ts apps/desktop/tests/smoke/flow-control-authoring.spec.ts apps/desktop/tests/smoke/flow-run-experience.spec.ts apps/desktop/tests/smoke/workspace-bundle.spec.ts --workers=2
```

PASS：Chromium 12 项。覆盖布局、变量草稿重开与取消/丢弃、Flow API/SSH/SQL 编辑、condition/probe、
run/history 和英中文 workspace bundle/security UI。后者是浏览器 mock 的消费测试，不能当作 MCP
权限执行或真实导入集成验证。没有修改 UI 组件或样式。

## 剩余风险与建议

- NOT RUN：Rust/native Tauri、真实 SSH/数据库、Cloud Sync、MCP 安全执行集成。生产 Rust、
  凭据持久化、协议、schema 和公共命令包装没有修改；本轮不作这些路径的新验证声明。
- Workspace mock 仍是现有的简化内存行为：例如 workspace 删除没有实现生产级关联记录清理，
  resolver 与 list 对 deletedAt 的处理并不统一。没有为重构顺便改变这些行为，需单独制定 mock
  与后端兼容契约后再修正；secret fixture 也不代表 OS credential store 行为。
- Flow 的 environmentInputErrors 仍按 name/key 求差集；SQL/ref/defaults 原规则原样保留。
  是否改为 stepId 去重、强化参数规则，属于另一个行为变更任务，不能混入本轮结构调整。
- 下一轮优先 API collection/folder mock 的职责与移动/排序契约，或 SSH transcript 的纯事件归约。
  不建议立刻对 session hooks 或 MCP/Cloud Sync 做类似的机械 dispatcher 拆分。
