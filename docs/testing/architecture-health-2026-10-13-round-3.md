# 第三轮可维护性重构：API 集合组织与 SSH 任务日志

## 基线与优先级

延续 [全仓评估](architecture-health-2026-10-12.md) 和
[第二轮验证](architecture-health-2026-10-13.md)。本轮开始工作区干净，直接在 main 修改。
重新 fetch 后 origin/main 为 `f4c03794`，HEAD 为上一轮本地提交 `12f78ed7`；
已验证 origin/main 是 HEAD 的祖先，保留尚未推送的上一轮提交。

本轮重新执行全仓 ESLint、超长文件扫描，并审阅候选实现、调用方、测试和 Git 历史。
这是延续全仓评估的重点落地，不是重新逐行审计所有文件。
基线为 0 errors / 35 warnings，87 个超长文件，0 blocking。

| 投入优先级 | 证据与判断 | 决定 |
| --- | --- | --- |
| P1，本轮 | API collections mock：15 条命令共享一个函数，复杂度 96、有效函数长度 216。集合生命周期、目录树、请求移动和排序混合；已有消费者测试，但缺少树级联和失败后的状态契约。 | 拆开操作职责，集中重复排序规则，保留原参数默认值和返回结果。 |
| P1，本轮 | SSH transcript：仅 121 物理行，但复杂度 37；事件归并、标题、传输格式化和完成信息混合，原来仅 2 项直接测试。 | 同文件内分离事件处理和格式化，补多步交错、缺失字段、完成与迟到事件覆盖。 |
| P2，下一轮专项 | `useSshTaskRunSession` 同时管理运行表单/环境输入、事件批处理订阅、运行 mutation、历史缓存。已有 input/event 纯模型拆分有价值，但没有该 hook 的直接测试；清理和隐藏面板缓冲互相约束。 | 先补工作区切换、订阅晚完成、卸载、隐藏/恢复 Tasks 和运行完成的时序矩阵，再决定拆分边界。 |
| P2，后续 | API request mock 的执行、取消和存储仍集中，复杂度 87；SSH mock 复杂度 91。 | 优先区分同步记录操作与异步执行；不能把本轮同步路由方式机械套到取消/事件生命周期。 |
| 保留 | MCP/Cloud Sync、长集成测试、声明型 schema、共享 TreeView。 | 安全/协议边界需要专项验证；长测试、显式 allowlist 和交互组合不因行数就判为设计缺陷。 |

基线最近 100 个仓库提交中，API collections 修改 1 次，transcript 与 session hook 均为 0 次。
因此不把它们描述为近期高频热点。本轮优先级来自职责集中、薄弱的直接回归保护和低副作用边界；
transcript 的历史曾涉及任务输出和命令回显功能，顺序与呈现语义值得明确保护。
没有证据支持重新划分 feature 包，或引入跨 feature 的通用执行/CRUD 框架。

## 结构变化与兼容性

- `api-collections.ts` 只注册、查找和调用命令；15 个命令名称与原实现一致。
  根据 handler 是否存在判断命令是否处理，保留 folder delete 的合法 `undefined` 返回。
- `api-collection-lifecycle.ts` 负责集合列表、创建、重命名、级联删除和浏览器导入/导出取消结果。
- `api-collection-folders.ts` 负责目录创建、重命名、删除、移动、列表和重排。
- `api-request-organization.ts` 负责已有请求的移动和重排，不承担请求执行、取消、鉴权或脱敏。
- `api-collection-order.ts` 仅共享两种记录的 scoped sibling 顺序更新；调用方仍负责验证父级和返回结果。
  没有新增全局框架、mockStore 状态或公共包导出。
- `task-run-transcript.ts` 在原文件内拆为 step/output 事件处理、标题/传输行格式化和完成详情。
  每次调用新建局部归并状态；没有 React effect、持久状态、事件订阅或协议变更。

明确保留的细节：

- workspace 隔离、名称 trim/default、原对象更新、时间戳、目录后代级联和错误文本。
- move 先设置目标父级再计算 sortOrder，因此把被移动对象自身计入计算；没有顺便修正。
- reorder 接受部分和重复 ID，按输入逐项更新；后续 ID 无效时前面的修改保留。
  folder 返回集合内排序后的目录，request 返回该 workspace 所有请求的原存储顺序。
- 日志按接收顺序处理，保留 React key、stream、空输出过滤、重复 running 标题和隐式标题。
- 每步只保留最新 transfer，完成时先输出 transfer 再输出 meta/error；未完成项按首次出现顺序追加。
  已完成步骤收到迟到 transfer 仍产生 live 行，零耗时和 exit 0 不被忽略。
- labels 仍由调用方提供，用户界面文案、交互、公共接口、存储格式、凭据和 Rust 执行链不变。

## 指标对比

统计使用相同仓库 ESLint 配置及超长文件检查器；没有修改阈值、baseline、排除项或禁用规则。

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| ESLint errors / warnings | 0 / 35 | 0 / 32 |
| complexity warnings | 13 | 11 |
| max-lines-per-function warnings | 19 | 18 |
| max-lines / unused-vars / set-state-in-effect | 1 / 1 / 1 | 1 / 1 / 1 |
| 超长文件 | 87 | 87 |
| Critical / P0 / P1（长度分类） | 0 / 20 / 67 | 0 / 20 / 67 |
| blocking / grandfathered | 0 / 0 | 0 / 0 |
| API collection dispatcher 复杂度 | 96 | 2 |
| transcript 入口复杂度 | 37 | 7 |

本轮修改和新增的 TypeScript 文件没有 Lint 告警。
拆分后的 API 操作函数最大复杂度为 10，transcript 内所有函数最大为 7。
入口复杂度下降代表决策被归属到具体操作，不代表业务分支凭空消失。

按仓库 `countLines` 的物理行口径，API 实现从 250 行变为
28（路由）+ 78（集合）+ 118（目录）+ 43（请求组织）+ 24（共用排序）= 291 行。
transcript 从 121 行变为 141 行。总实现增加 61 行，换取独立操作边界和集中格式化/排序规则；
本轮不以代码压缩或减少超长文件数量作为收益。

## 验证证据

新增 16 项单测：API collections 10 项、SSH transcript 6 项。
API 测试通过实际 mockInvoke 路由，使用内存 fixture 并恢复 store/active workspace/时钟。

| 检查 | 结果 |
| --- | --- |
| 重构前运行两个目标测试文件 | PASS：2 文件、18 项（包括原有 2 项）。 |
| `pnpm exec vitest run packages/command-client packages/api-client packages/ssh-terminal` | PASS：67 文件、413 项。 |
| `pnpm test` | PASS：174 文件、1111 项；上一轮基线为 173 文件、1095 项。 |
| transcript 一次性差分检查 | PASS：读取 HEAD 原实现与工作区实现，TypeScript transpile 后使用相同 formatFileSize/labels，对比 5000 组确定性事件序列的完整输出。固定 seed 12345，涵盖多种 kind/status/step/stream/空值及传输数据组合；不替代真实事件集成。 |
| `pnpm run build` | PASS：TypeScript + Vite；既有 >500 kB chunk 提示仍在。 |
| `pnpm exec eslint . -f json -o <temporary-report>` | PASS：0 errors / 32 warnings。 |
| `pnpm run check:large-files --json --top 300` | PASS：87 个提示、0 blocking。 |
| `git diff --check` | PASS。 |

浏览器验证命令：

```text
pnpm exec playwright test apps/desktop/tests/smoke/api-client-layout.spec.ts apps/desktop/tests/smoke/sidebar-consistency.spec.ts apps/desktop/tests/smoke/ui-smoke.spec.ts --workers=2
```

PASS：Chromium 5 项；覆盖 API 工作台布局、英中文资源侧栏、Flow/SSH 长列表滚动和模块切换。
任务日志的事件语义由单测及差分检查覆盖；浏览器 smoke 不声称覆盖真实远端任务执行。

## 剩余风险与下一步

- NOT RUN：Rust/native Tauri、真实 SSH、Cloud Sync、MCP 安全执行集成。本轮没有改动这些执行边界，
  浏览器 mock 和 transcript 测试不构成它们的新验证证据。
- mock 排序的自包含计算、非原子更新和后端是否一致，需要独立兼容性任务验证；本轮已锁定原行为，
  不在结构重构中隐式改变。导入/导出仍是取消 fixture，不复制 Rust 协议。
- 下一轮最值得投入的是 SSH task session 的异步时序保护，再分离运行表单/环境输入与事件缓存生命周期。
  单纯继续拆巨型 UI 或清除警告不能代替这些回归保护；安全协议和 outbox 仍留专项处理。
