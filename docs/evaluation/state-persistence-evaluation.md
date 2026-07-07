# API / DB / SSH 页面状态保存评估（切换页面 & 切换工作区）

> 评估范围：三个业务模块在「模块切换（page switch）」与「工作区切换（workspace switch）」两个维度下的 UI 状态保存行为，以及跨重启的持久化程度。
> 评估依据：源码核对（`packages/*`）+ 架构文档（`AGENTS.md`、`package-boundaries.md`、`data-storage.md`、`security-model.md`、`design.md`、`interaction-guidelines.md`）。
> 本次仅评估，**未修改任何代码**。

---

## 0. 结论速览

| 维度 | API Client | Database | SSH Terminal |
| --- | --- | --- | --- |
| 模块切换（切到别的页面） | ✅ 保留（hidden 保活） | ✅ 保留（模块 store 存活） | ⚠️ 保留（store 重放，但 xterm 实例被销毁重建） |
| 工作区切换 | ❌ **全部未保存草稿/标签/响应丢失** | ✅ 保留（按 workspace 隔离） | ⚠️ **滚动缓冲清空后从后端异步回填**（瞬时丢失/闪烁，选中态重置） |
| 跨重启 | ❌ 不持久化 | ❌ 不持久化（仅 shell 布局恢复） | ❌ 不持久化（仅 shell 布局 + 后端 PTY 存活） |

**核心问题**：现状严重不对称。Database 在两个维度都正确；API 在「切换工作区」时直接丢失全部进行中工作（违反文档「切换须保留脏内容」）；SSH 在「切换工作区」时丢弃内存滚动缓冲再回填，存在瞬时空白与选中态丢失。**前端业务状态完全没有持久化**，唯一落库的是 shell 布局（按工作区隔离）。

---

## 1. 现状事实（已核对源码）

### 1.1 app-shell 的模块挂载方式（`packages/app-shell/src/DesktopApp.tsx`）

三个模块的挂载方式**不对称**，这是后续所有状态行为差异的根因：

- **API Client（始终挂载，CSS 隐藏保活）**
  ```tsx
  <div className={activeTab.kind === "api" ? "h-full" : "hidden"}>
    <ApiClientPage
      key={activeWorkspace.id}   /* ← 切换工作区即整体重挂载 */
      workspaceId={activeWorkspace.id}
      ...
    />
  </div>
  ```
- **SSH Terminal（条件挂载，非激活即卸载）**
  ```tsx
  {activeTab.kind === "ssh" && activeWorkspace && (
    <TerminalPage workspaceId={activeWorkspace.id} ... />
  )}
  ```
- **Database（条件挂载，非激活即卸载）**
  ```tsx
  {activeTab.kind === "database" && activeWorkspace && (
    <DatabasePage workspaceId={activeWorkspace.id} ... />
  )}
  ```

含义：
- 切到 SSH / Database 时，**API 仍挂在 DOM 里只是 `hidden`** → API 的 React 内存状态得以保留（满足「模块切换保留脏内容」）。
- 切离 **SSH / Database** 时，组件被**真正卸载**，其组件内 `useState` 销毁；它们能「记住」东西，是因为状态搬到了模块级 zustand store。

### 1.2 状态持有位置与隔离方式

| 模块 | 持有方式 | 是否按 workspaceId 分区 | 是否持久化 |
| --- | --- | --- | --- |
| API 请求草稿 / 打开标签 / 响应 | `ApiClientPage` 内 `useState`（`useApiRequestTabs`：`useState(() => createNewRequestTab(emptyApiTabsState(workspaceId), "new:1"))`） | 否（仅把 workspaceId 写进草稿，状态本身不分片） | 否 |
| DB 查询/表标签 / 草稿 SQL / 查询结果 | 模块级 zustand `useDatabaseTabStore`（`byWorkspace: Record<string, ...>`） | **是** | 否 |
| DB 连接会话状态 | 模块级 zustand `useDatabaseConnectionStore`（`byWorkspace`） | **是** | 否 |
| SSH 会话事件 / 滚动缓冲 / 选中态 | 模块级 zustand `useTerminalStore`（**单一非分区槽位** `workspaceId`） | **否** | 否 |
| Shell 布局（激活模块 / 选中 ID / 侧栏） | `workspace-core` store + 后端 | 是（按 workspace 落库） | **是**（经 `workspace_layout_update` → SQLite） |
| i18n / 主题 | `packages/ui` localStorage | 不涉及 | 是 |

### 1.3 工作区切换流程

入口：`AppTitleBar.onActivateWorkspace` → `activateWorkspaceMutation` → Tauri `setActiveWorkspaceCommand` → store `setActiveWorkspace({ activeWorkspaceId })`（`workspace-store.ts`）。

`activeWorkspaceId` 改变后：
- `activeWorkspace` 重新派生；`workspaceLayoutQuery` 以 `activeWorkspace.id` 为 key 重新拉取并 `hydrateLayout` 恢复该工作区的 shell 布局（含恢复当前激活模块）。
- 各业务模块**各自**处理，没有统一的「切换工作区 → 重置业务 store」逻辑：
  - **API**：`key={activeWorkspace.id}` 随 id 变化 → `ApiClientPage` 整体重挂载 → `useState` 重置为 `emptyApiTabsState` → **全部草稿/标签/响应丢失**。
  - **Database**：store 按 `workspaceId` 取不同切片 → 切换只是换切片，**保留且不串号**。
  - **SSH**：`TerminalPage` 在 `workspaceId` 变化时调用 `activateWorkspace(workspaceId)`（`terminal-state.ts:58-75`），该函数**整体清空** `terminalEvents / activeSessionId / dismissedSessionIds / splitMode ...`，随后从后端 `getSshSessionHistory` 异步回填。

### 1.4 持久化盘点（全仓扫描）

- 唯一命中 `localStorage` 的是 `packages/ui`（i18n 语言、主题），与业务无关。
- 无任何 `zustand/middleware`、`persist`、`sessionStorage` 命中。三个 feature store 均**无持久化**。
- 唯一真正的「持久化」路径是 shell 布局：经 `command-client` 的 `getWorkspaceLayout / updateWorkspaceLayout` → Tauri `workspace_layout_update`，落库到 `workspace-local / workspace-engine`。内容为 `WorkspaceLayout`（activeTabId、tabs、选中 ID、sidebarCollapsed、workspaceId）——**仅 shell 级，不含 API 请求标签 / DB 查询标签 / SSH 会话**。

---

## 2. 与文档意图的差距（Gap）

| 文档要求 | 现状 | 差距 |
| --- | --- | --- |
| 「切换 tab 或模块必须保留脏内容」（`interaction-guidelines.md`） | API：模块切换保留 ✅；**工作区切换丢失** ❌。SSH：模块切换靠 store 重放 ⚠️（xterm 实例销毁重建）。 | API 在工作区切换维度**直接违反**。 |
| 「布局/标签恢复状态属于工作区所有」（`data-storage.md`） | shell 布局按 workspace 隔离并持久化 ✅；业务标签（API/DB/SSH）**不在布局快照里**，且无持久化 ❌。 | 所谓「按工作区隔离的标签恢复」仅对 shell 成立，对业务标签不成立。 |
| 「每个业务记录必须带 workspace_id」（`data-storage.md`） | 后端记录都带 ✅；前端草稿状态无持久化故 N/A，但 **API 用「整段清空」而非「按工作区分片」** 来处理切换，设计上不如 DB 的隔离模式合理。 | API 的切换语义应改为隔离而非丢弃。 |
| 「design.md 关键词：Context retention / Persistently visible state」 | 仅 Database 真正满足；API 工作区切换、SSH 工作区滚动缓冲均破坏可见状态连续性。 | 连续性目标未达成。 |

---

## 3. 应该怎么样（评判标准）

综合 `design.md`、`interaction-guidelines.md`、`data-storage.md`、`security-model.md`，目标态应为：

1. **模块切换**：不得丢失任何进行中的工作 —— 脏草稿、打开标签、查询结果、终端滚动缓冲都应保留。三个模块都应达到 Database 的水平。
2. **工作区切换**：每个工作区的打开标签/草稿必须**隔离且保留**（不丢、不串号）。达到 Database 的水平。
3. **跨重启恢复**：shell 布局已恢复；业务标签是否恢复是产品决策，但「标签恢复状态」已被文档定义为工作区所有 → 应可恢复。区分两层：
   - *打开标签集合*（哪些请求/查询/会话是开着的，哪些脏）= 属于「tab restore state」，**应持久化**。
   - *未保存草稿内容*：文档说「自动保存不适用于请求/SQL/凭据」，但「保留脏内容」又要求不丢。务实解读（参考 VS Code hot-exit）：允许持久化未保存草稿，但需**脱敏**。这是产品决策点。
4. **安全约束**：若持久化未保存草稿，header/body 可能含 `authorization / x-api-key / token / cookie` 等 → 落库前**必须按 `security-model.md` 脱敏**，只存 `credential_ref`，绝不存原始 secret。

---

## 4. 方案与优劣势对比

两个正交关注点：**（a）模块切换存活**；**（b）工作区切换隔离**；**（c）可选：跨重启持久化**。

### 方案 A：统一为「模块级 + 按 workspaceId 分区的 zustand store」（即 Database 模式）
将 API 的组件内 `useState` 上提到模块级 `useApiRequestTabStore`，按 `workspaceId` 分区；移除 `key={activeWorkspace.id}`。SSH 的 `useTerminalStore` 同样改为 `byWorkspace` 分区（参照 `useDatabaseConnectionStore`），`activateWorkspace` 改为切分片而非清空。app-shell 挂载方式保持不变。

- **优势**：① 同时修复「模块切换存活」（store 在模块级）与「API 工作区丢失」（分区后无重挂载）；② 与已有 Database 模式一致，**架构风险低、无新增依赖**；③ 不动后端、不改命令总线；④ 改动量小（API 上提 state、SSH 改分区）。
- **劣势**：① 仍纯内存，**无跨重启恢复**；② 所有工作区的标签常驻内存（v0.1 可接受，后续可加 LRU 淘汰）。

### 方案 B：全部 keep-alive 挂载（hidden + 去 key）
让 SSH/Database 也始终挂载 + `hidden`，并移除 API 的 workspace key，依赖组件级 state。

- **优势**：app-shell 侧改动最小；SSH 无需 store 迁移。
- **劣势**：① 仅去 API 的 key 还不够 —— 组件级 state **不按工作区分片**，切工作区会把 A 工作区草稿与 B 混在一起（**串号**），隔离仍需分区；② SSH 始终挂载意味着 xterm 实例常驻（PTY/内存成本）；③ 不解决重启恢复。

### 方案 C：业务标签/草稿落库后端（完整 hot-exit）
扩展 `workspace_layout` 或新增表（如 `api_open_tabs / db_open_tabs / ssh_open_sessions`，均带 `workspace_id`），在激活工作区/启动时 hydrate；草稿变更做防抖合并写入。

- **优势**：① 真正跨重启/崩溃恢复；② 契合「tab restore state 属工作区所有」「local-first SQLite 为真相源」；③ 满足「保留脏内容」最强形态。
- **劣势**：① 改动大（新 schema + 命令总线命令 + 防抖持久化，草稿每次按键都变）；② 与「自动保存不适用于请求/SQL 默认」存在**产品张力**，需先决策是否存未保存草稿；③ 草稿可能含 token → 落库前**必须脱敏**，增加安全面与复杂度；④ v0.1 风险高。

### 方案 D：混合 —— A 现在做，C 后续做（推荐）
- **Phase 1（v0.1）**：采用方案 A，修复真实的数据丢失 bug（API 工作区丢失、SSH 滚动缓冲 churn），统一到 Database 模式，零新增依赖、低风险。
- **Phase 2（v0.1 之后）**：待持久化 schema 与脱敏策略就绪，再上方案 C 的业务标签恢复。
- **优势**：① 用最小成本先拿到「正确性」（不丢、隔离）；② 把高风险的持久化工作后置并隔离；③ 符合「先发 v0.1、后续文档化遗留项」的节奏。
- **劣势**：① Phase 1 仍无重启恢复（已知、已文档化的缺口）；② 两阶段交付。

### 方案 E：仅修 API 重挂载 bug（最小改动）
仅移除 `key={activeWorkspace.id}` 并把 API state 提到 `byWorkspace` store，不动 SSH 的切工作区清空逻辑。

- **优势**：最小改动修最严重的 bug。
- **劣势**：① SSH 切工作区仍清空滚动缓冲/重置选中态，不彻底；② 与统一架构目标不一致，留下第二个不对称点。

### 方案对比矩阵

| 方案 | 修模块切换 | 修 API 工作区丢失 | 修 SSH 工作区 churn | 跨重启恢复 | 新增依赖 | 风险 | 工作量 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A 分区 store | ✅ | ✅ | ✅ | ❌ | 无 | 低 | 中 |
| B keep-alive | ✅ | ❌（会串号） | ⚠️ | ❌ | 无 | 中 | 低-中 |
| C 落库后端 | ✅ | ✅ | ✅ | ✅ | 无（但新 schema） | 高 | 大 |
| D 混合(A→C) | ✅ | ✅ | ✅ | Phase2 | 无 | 低→中 | 中→大 |
| E 仅修 API | ✅ | ✅ | ❌ | ❌ | 无 | 低 | 小 |

---

## 5. 推荐落地路径（若后续执行，本次未做）

**推荐方案 D。**

**Phase 1（v0.1，方案 A）**
1. 新增 `packages/api-client/src/model/api-request-tab-state.ts`：`create` 模块级 store，顶层 `byWorkspace: Record<string, ApiTabsState>`；把 `useApiRequestTabs` 内的 `useState` 改为对该 store 的选择器读写，所有增删改带 `workspaceId`。
2. 移除 `DesktopApp.tsx:197` 的 `key={activeWorkspace.id}`。
3. 将 `useTerminalStore` 改造为 `byWorkspace` 分区：`activateWorkspace` 改为「取对应分片」而非「整体清空」；保留 `terminalSearchAddon` 全局。
4. 保持 app-shell 挂载方式不变。
5. 验证：模块切换 / 工作区切换后，API 草稿、DB 标签、SSH 滚动缓冲均保留且不串号。

**Phase 2（后续，方案 C，需先定产品决策 + 脱敏策略）**
1. 新增工作区级业务标签表（带 `workspace_id`），纳入 `data-storage.md` 的 syncable 候选。
2. 在命令总线新增 `open_tabs_get / open_tabs_update`（脱敏后写入）。
3. 草稿变更防抖（如 350ms，复用 `useLayoutPersistence` 的防抖模式）合并写入。
4. 落库前对 request draft 的 auth/header/body 应用 `security-model.md` 脱敏规则，绝不留原始 token。

**明确不做的**
- 不要用 `window.confirm` 做危险操作确认（`interaction-guidelines.md` 禁止）。
- 不要把业务状态写进 `packages/app-shell` 或 `packages/ui`（包边界禁止）。
- 持久化未保存草稿前**必须先过脱敏**，否则违反 `security-model.md`。

---

## 6. 待确认的产品决策点

1. 未保存草稿是否要「hot-exit」持久化（方案 C）？还是仅持久化「打开标签集合」、草稿只在内存保留（方案 A/D）？
2. SSH 模块切换时，是否接受「xterm 实例销毁 + store 重放」的当前行为，还是改为常驻 keep-alive（更顺滑但更耗资源）？
3. 跨重启的业务标签恢复，优先级是否排在 v0.1 之外？
