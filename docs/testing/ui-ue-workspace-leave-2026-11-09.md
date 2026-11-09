# UI/UE 收尾：Flow 离开保护与环境切换一致性 — 2026-11-09

本轮承接 [核心工作流 UE 检查](core-workflow-ue-2026-11-09.md)，保留 C2 配色、布局、组件及交互语言。
开始时分支为 `main`，工作区干净；修复阶段未切分支、提交或推送。后续按用户指示完成提交前审核，在 `main` 提交，不推送。没有增加依赖、Rust 命令、数据库结构或草稿持久化。

## 实际问题与修复

### Flow 离开保护

确认原实现仍在 `DesktopWorkspaceContent` 和 `FlowPage` 按 workspace key 挂载，`useFlowEditor` 的编辑状态保存在组件内。
旧实现中，新建或编辑 Flow 后，即使已经隐藏到 API 模块，切换 Workspace 仍直接卸载编辑器，浏览器回归找不到离开确认。

采用 **取消 / 放弃并离开** 策略，取消获得默认焦点，Enter 执行取消。复用 Shell 已有的变量编辑导航保护和共享 `ConfirmDialog`：

- Flow 仅上报 dirty 布尔值；definition、未解析 JSON、敏感输入仍在 Flow 组件内。现有 `schemaValidity` / `stepValidity` 已将解析失败标为 dirty，不以序列化后的 definition 比较替代它。
- 同 Workspace 跨模块继续保留编辑器；隐藏 Flow 的 dirty 状态仍阻止 Workspace 切换。保留 workspace key 和工作区隔离。
- Flow 和变量编辑器同时有未保存内容时，一个离开确认涵盖两者。只有跨模块时，仍只按原规则确认变量编辑器，Flow 不被重置。
- 普通 Workspace 菜单、扩展 `activateWorkspace`、导入完成回调均走 `handleActivateWorkspace`。新建和删除 Workspace 会在后端改变活动 Workspace，因此在发出生命周期命令**之前**通过同一个离开门禁。
- Workspace bundle 导入本身不设置活动 Workspace，完成后的激活回调有保护；Cloud Workspace 打开使用同一扩展激活回调。启动/后端状态同步继续使用 `useWorkspaceInit`，未改变外部同步协议。

本轮**不提供保存后切换**。已核对 `useFlowEditor.save()` 经 `perform()` 内部捕获错误，不能用 Promise fulfilled 判断成功。原有显式 Save 的成功、普通失败、`FLOW_REVISION_CONFLICT` 均有测试：只有成功清除 dirty；失败/冲突保留内容和离开保护。不引入自动保存，也不将输入写入新的缓存。

### 环境切换

使用每个浏览器上下文独立的 fixtures，环境 a1/a2/a3 与另一个 Workspace 的 b1，手动控制激活 Promise 的完成/失败。所有 API 网络请求由 Playwright 拦截 `https://race.example.test/**`，记录最终解析地址，不访问外部服务。

| 场景 | 旧实现证据 / 判断 | 最终行为 |
| --- | --- | --- |
| 激活未完成立即 Send | 原发送入口不检查激活中状态，保护断言 FAIL。 | 标题显示“正在切换环境”；发送处理函数同步读取当前 Workspace 的 mutation cache，拒绝 Send / Retry / 快捷键路径并解释原因。不会排队自动发送。 |
| 连续 a2 → a3，完成顺序反转 | 用基线 `15aacfa6` 版本的 `useDesktopWorkspace` 复跑：a3 先完成，a2 后完成；实际捕获 `/a2/health`，期望 `/a3/health` 的回归 FAIL。这里是最后一次选择被覆盖，不声称旧 UI 显示 a3 时请求却发往 a2。 | 同 Workspace 的激活使用 React Query mutation scope 串行执行，pending 包含等待中的选择；最后一次选择 a3 完成后实际请求 `/a3/health`。不同 Workspace 的队列独立。 |
| 较早启动的环境列表后返回 | 组件测试控制 list Promise，激活成功后旧 list 不应把缓存改回 a1。 | 写入激活结果前取消该 Workspace 仍在进行的列表查询，避免旧列表覆盖已完成选择。 |
| 激活失败、重试 | 浏览器发现 Shell hook 位于其返回的 FeedbackProvider 外，错误 handler 使用空反馈接口，失败提示不可见。 | 将 Provider 放到 Shell hook 上层；失败展示原有本地化错误与详情，保留已确认的活动环境。失败后请求到 a1；再次选择 a2 成功后请求到 a2。队列中的失败不阻塞后续选择。 |
| 激活期间切 Workspace | 原 Shell onSuccess 已使用 mutation input 的 workspaceId。未复现该路径写坏新 Workspace 的缓存，不声称此处发现新缺陷。 | A 的延迟回调只更新 A；B 显示 b1，实际请求 `/b1/health`；返回 A 显示 a3，实际请求 `/a3/health`。旧 Workspace 的 pending 不阻塞 B。 |

`useWorkspaceEnvironments.activateMut` 同步采用相同 key/scope，并以启动时 Workspace 身份写回缓存，保留其现有调用接口。UI 执行保护放在 API feature 的 `sendTab` 内，而非只禁用按钮。不会重写已经发送的请求或运行任务。

## 其他执行入口核对

- API 的发送、响应 Retry、键盘发送和历史打开后的发送共用 `sendTab`。Rust API script 路径先解析环境 ID，再用于请求及脚本上下文；本轮未改命令链。
- SSH Task：`useTaskRunDialogState` 通过准备/确认界面保存明确的环境 ID、变量来源、环境版本和任务版本；代次/Workspace 检查拒绝过期准备结果。已有确认上下文不跟随顶部选择，正在运行任务不重新选择目标。
- Flow：`openRun` 快照默认环境，`useFlowRuns` 传明确的环境 ID（null 为 workspace-only）和 definition revision。保留未加载默认环境、失败重试、手动环境选择和历史重跑的现有语义。
- Database 查询、交互式 SSH 终端使用已选择的连接/会话，不在每次执行时隐式重选顶部活动变量环境。Flow / SSH Task 中使用变量的动作仍遵从上述显式上下文。
- 验证了现有 Flow en / zh-CN 环境快照浏览器流程、SSH 历史参数重跑及相关组件测试；没有为未复现路径新增执行状态机。

## 验证记录

| 检查 | 结果 |
| --- | --- |
| 旧实现回归 | **FAIL（预期复现）**：隐藏 Flow 无离开确认；切换中 Send 无门禁；两次环境激活并发启动；反向完成后实际地址为 a2 而非最后选择的 a3。 |
| 相关 Vitest | **PASS：40 文件、392 项**。涵盖 Flow 新建/已有/无效 JSON、显式保存成功/失败/冲突、生命周期及扩展导航入口、API 实际发送门禁、环境队列/旧列表/跨 Workspace、原变量保护、SSH/Flow 快照、i18n。 |
| Chromium 浏览器回归 | **PASS：7 文件、22 项**。新增文件 8 项；另覆盖上一轮 API/DB/SSH 恢复、跨模块保留、Flow 环境/运行、SSH 重跑、Workspace bundle en / zh-CN。旧实现对照复跑后，恢复修复代码并再次运行新增 8 项，全部 PASS。 |
| Desktop TypeScript | **PASS**：`pnpm --filter @unfour/desktop exec tsc --noEmit`；最终直接调用本地 `tsc.cmd --noEmit` 再次通过。 |
| ESLint（变更 TS/TSX） | **PASS（0 errors）**；现有 API hook 长度与 Desktop workspace hook 复杂度警告仍存在，已对 HEAD 用 stdin 检查确认原先也存在，未为消除警告做无关拆分。 |
| `pnpm run build` | **PASS**；保留已有 >500 kB chunk 提示。 |
| 大文件检查 | **PASS（0 blocking）**；报告仓库现有大小分级。 |
| `git diff --check` | **PASS**。 |

运行中遇到的检查失败单独记录：pnpm 多次在启动阶段返回 `unable to open database file`；直接运行 Vitest 曾因沙箱临时目录 rename 的 `EPERM` 在收集阶段失败（0 tests）。旧代码复跑也曾因本地服务器启动/端口及进程等待未完成而中止，临时文件已清理、修复代码已恢复。最终通过授权的本地隔离运行取得上表旧实现真实地址 FAIL 证据；这些工具失败不计为产品测试通过。新增浏览器用例曾有 label 匹配、同模块点击折叠侧栏和原有“Discard changes”按钮名称的定位错误，修正定位后通过。

可复现命令：

```powershell
pnpm exec playwright test apps/desktop/tests/smoke/workspace-leave-environment-race.spec.ts apps/desktop/tests/smoke/core-workflow-context.spec.ts apps/desktop/tests/smoke/core-workflow-recovery.spec.ts apps/desktop/tests/smoke/flow-environment.spec.ts apps/desktop/tests/smoke/flow-run-experience.spec.ts apps/desktop/tests/smoke/ssh-task-rerun.spec.ts apps/desktop/tests/smoke/workspace-bundle.spec.ts --workers=1
pnpm --filter @unfour/desktop exec tsc --noEmit
pnpm run build
git diff --check
```

## 验证边界与剩余风险

- **NOT VERIFIED**：Tauri 原生窗口、原生 command IPC 的真实乱序/失败、Rust 执行、真实网络、数据库及 SSH 服务。本轮没有执行生产操作，也没有将浏览器 mock PASS 当作原生 PASS。Rust 代码未修改，未运行 Cargo/native 构建或集成测试。
- 此保护面向应用内 Workspace 导航和生命周期入口；不是崩溃恢复、关闭窗口保护、后端外部删除工作区恢复或完整草稿持久化。
- 显式选择放弃后，Workspace 切换仍沿用原有乐观激活/回滚语义；若激活失败，已经允许放弃的 Flow 草稿不保证恢复。
- 环境串行/门禁覆盖此 UI 的激活请求，不是跨进程或 MCP 并发修改的分布式锁；外部工具同时改变活动环境仍需原生隔离测试。
