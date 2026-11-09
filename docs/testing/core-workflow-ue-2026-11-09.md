# 核心工作流 UE 检查与修复 — 2026-11-09

本轮完成 **3 项已复现问题的修复**，保留 C2 配色、字体、导航和组件语言。
没有新增业务功能、依赖、后端协议或持久化模型；不包含合并或发布。
开始时 `git status --short` 为空。

## 验证边界

- 检查对象：当前源码 + Vite 运行的真实 React 界面，使用 Chromium / Playwright 操作。
- 数据与执行：每个浏览器上下文独立的 command-client browser fixtures。Workspace A/B 标为 test；其他复用的 Default / Atlas 场景也是浏览器内的临时测试数据，界面 DEV 标签不代表访问开发服务器。
- API 仅访问 Playwright 拦截的 `https://ue.example.test/**`；DB 命令返回合成结果或注入连接错误；SSH 仅注入任务事件；Flow 仅返回合成执行记录。未连接生产环境或真实数据库/SSH 主机。
- **NOT VERIFIED**：Tauri 原生窗口、Rust 执行、真实网络超时、真实 DB/SSH 服务、持久化和系统凭据。浏览器 PASS 不代表这些路径已验证。
- 查看了修复前 API 状态及修复后 API、Database、SSH 截图；行为验收以交互断言为主。

## 操作前后

| 修复 | 原操作路径与已验证问题 | 修改方案 | 修改后验证 |
| --- | --- | --- | --- |
| API：诊断页签内直接理解失败并重试 | 成功发送 → 切到 Tests → 修改 URL → 网络失败。响应区仅有简略状态和“没有测试记录”；Request / Tests / Console 的条件渲染排除了失败详情及 Retry，必须自己切回 Body。 | 在这三个页签上补充紧凑错误提示与现有 Retry 操作；保留原页签、脚本输出和请求草稿。沿用现有发送处理，使用当前编辑后的草稿。 | 浏览器确认 Tests 上直接看到错误和 Retry；修改 URL 后 Retry 发往新地址并成功，提示随状态消失。组件覆盖三个页签、发送中禁止重复重试、前置脚本失败且控制台仍可查看。 |
| Database：Messages 不再把执行失败呈现为“无消息” | 编辑 SELECT → Run All → 命令级连接失败 → 点击 Messages。Results 有错误，Messages 却仅显示无消息，无法在此继续查错。 | Messages 复用现有错误/加载展示；保留语句级批处理消息和确认面板，未新增 SQL 执行入口或绕过确认。 | 修复前浏览器断言失败：Messages 中找不到已出现的连接错误。修复后可见错误详情；修改 SQL 后 Run All 成功，旧错误消失。确认仍显示待确认 SQL；已有失败/跳过语句消息测试通过。 |
| SSH Task：阅读日志不再被新输出打断 | 运行任务 → 输出超过一屏 → 向上找诊断 → 新输出到达。原 effect 每次强制滚到底，实测从 `scrollTop=0` 跳到约 2231px。 | 默认继续跟随；向上滚动时暂停跟随；提供“跟随最新输出”，回到底部后恢复自动跟随。输出区可键盘聚焦，点击恢复后焦点落到输出区；切换运行记录重置跟随状态。 | 浏览器注入 100 行输出，滚到顶部，再追加输出后仍在顶部；跨模块期间收到输出，返回后仍保留阅读位置；960×600 下恢复入口可达，点击后及后续输出均保持底部。组件覆盖另一条运行记录与焦点恢复。 |

API 和 DB 复用已有中英文文案；SSH 新文案使用共享 i18n 的 `ssh.tasks.run.followOutput` / `output`，提供 en / zh-CN。

## 五条检查路径与保留的现有能力

| 路径 | 运行界面与代码检查结果 | 本轮处理 |
| --- | --- | --- |
| Workspace / Environment 切换后执行 | Test A 的 A1 切 A2，等待标题更新后发送 `{{base_url}}/health`，捕获地址为 A2。切 Test B 后命中 B，返回 A 的 API 草稿、响应和 A2 环境保留。 | 保留已有隔离和变量解析。测试明确等待切换完成；不声称覆盖切换中并发执行。 |
| API 编辑、发送、响应、修改重试 | 发送失败在诊断页签被遮蔽；重试实际使用当前草稿。 | 修复第 1 项。 |
| Database / SSH Task 执行、结果、失败 | DB 的命令级失败在 Messages 丢失；SSH 日志抢滚动。SSH 历史重跑已有可编辑参数确认，对已删除连接和敏感输入已有处理。 | 修复第 2、3 项；保留现有重跑、安全确认、脱敏路径。 |
| Flow 编辑、运行、定位失败、修改重跑 | 现有失败定位、修改步骤、脏状态禁止运行、保存后版本增加、再次确认运行全部可用；合成失败 r1 修改等待时长后保存 r2，运行结果成功。环境快照、历史环境 ID、中英文窄窗口检查通过。 | 没有重复开发已有的定位或重跑功能。 |
| 跨模块继续工作 | API 草稿及响应、Flow 未保存名称跨模块保留；DB SQL 与结果跨模块保留；SSH 暂停阅读位置和缓冲输出返回后可继续看。 | 保留既有模块常驻机制；补充回归。Workspace 切换的 Flow 例外见下。 |

## 已验证但未修复

**Flow 未保存草稿跨 Workspace 丢失。** 在 Test A 创建并编辑未保存 Flow，跨模块仍保留；切 Test B 再返回 A 时，界面重新出现“Create your first Flow”。代码对应：

- `packages/app-shell/src/components/DesktopWorkspaceContent.tsx` 的 `FlowModule key={activeWorkspace.id}`；
- `packages/flow/src/FlowPage.tsx` 的按 workspace key 挂载；
- `packages/flow/src/useFlowEditor.ts` 的组件局部 draft 状态。

这是现有前端编辑会话生命周期问题，不是本轮引入，也没有声称已修复。完整解决需同时处理 workspace 隔离、未解析的编辑输入、历史/运行视图、隐藏弹层和后台订阅；只缓存已解析 definition 会仍然丢失输入，直接常驻所有页面也有弹层/订阅风险。本轮选择三个边界清楚的恢复问题，没有改成隐式保存或把业务状态放进 app-shell。应单独安排修复；当前切 Workspace 前需手动保存 Flow。

## 待验证假设

- 环境切换 mutation 未完成时继续发送/快速反复选择，可能存在目标理解或请求时序风险。`useDesktopWorkspace` 没有向标题栏暴露 pending 状态；本轮仅验证完成切换后的目标。需要可控延迟和原生命令时序实验，不能把源码疑点当成已复现的误执行。
- 原生 DB/SSH 的断网、重连和运行期间切 Workspace 行为仍需隔离服务测试。此处没有用前端成功状态替代真实执行结果。

## 检查记录

| 检查 | 结果 |
| --- | --- |
| 修复前 API / SSH 浏览器回归断言 | **FAIL（预期复现）**：Retry 不可见；滚动位置被强制改变。 |
| 修复前 DB Messages 浏览器回归断言 | **FAIL（预期复现）**：Messages 中错误详情缺失。首次脚本先遇到 fixture 表格点击/重复文案定位问题，修正测试定位后独立复现，未把测试问题当成产品问题。 |
| 修复后 `core-workflow-recovery.spec.ts` | **PASS**：3 个恢复流程，包含修改后重试和跨模块保留。 |
| `core-workflow-context.spec.ts` | **PASS**：2 个场景，Workspace/Environment 执行目标与 Flow 修改后运行新版本。初次返回 Workspace 的断言假设仍显示 API，实际恢复了保存的模块布局；改为明确打开 API 后验证草稿。 |
| `flow-environment.spec.ts` | **PASS**：2 项（en / zh-CN）。 |
| `flow-run-experience.spec.ts` | **PASS**：2 项，含窄窗口长错误定位与返回编辑。 |
| `ssh-task-rerun.spec.ts` | **PASS**：1 项，历史参数、删除连接和敏感输入。 |
| ResponseTabs / QueryResultPanel / TaskRunPanel / i18n 定向 Vitest | **PASS**：初轮 4 文件、30 项；提交前复查增加 1 项 SSH 运行记录切换回归，见下。 |
| 变更 TS/TSX 文件 ESLint | **PASS**：0 errors、0 warnings。中间出现的 ResponseTabs complexity warning 已通过将错误提示放回其所属 response view 消除。 |
| Desktop TypeScript `tsc --noEmit` | **PASS**：pnpm 命令曾通过；最终使用本地 `node_modules/.bin/tsc.cmd --noEmit` 在 `apps/desktop` 再次通过。 |
| `pnpm run build` | **PASS**；Vite 提示存在超过 500 kB 的 chunk，没有为此扩大重构。 |
| `git diff --check` | **PASS**。 |

pnpm 启动检查时曾间歇返回 `unable to open database file`（并行启动和后续单独启动都遇到过），这些命令没有完成。Lint 重试通过；最终类型检查直接调用仓库已有的本地 tsc，通过。记录实际失败，不将未执行的检查计为通过，也不将此归因为已确认的产品缺陷。

可复现浏览器检查（一次性隔离数据，无原生执行）：

```powershell
pnpm exec playwright test apps/desktop/tests/smoke/core-workflow-recovery.spec.ts apps/desktop/tests/smoke/core-workflow-context.spec.ts apps/desktop/tests/smoke/flow-environment.spec.ts apps/desktop/tests/smoke/flow-run-experience.spec.ts apps/desktop/tests/smoke/ssh-task-rerun.spec.ts --workers=1
```

截图由 `testInfo.outputPath(...)` 生成到忽略的 `test-results/`，是本机检查辅助材料，不作为业务执行证据或必须提交的交付物。验收重点是上面的操作前后变化。

## 提交前复查

- 发现并修复：长日志暂停跟随后，切到没有滚动事件的短日志，再返回原记录，旧的暂停状态可能被带回。原实现只推导当前显示状态，没有及时更新存储的 run ID。改为在运行记录身份改变时重置状态，不依赖浏览器 scroll 事件。
- 新回归先在原实现上 FAIL（旧的“跟随最新输出”按钮仍出现），用于确认问题；修复后与原有流程一起复验。
- 提交前复验 **PASS**：4 个 Vitest 文件共 31 项、5 个 Playwright 文件共 10 项；变更文件 ESLint、Desktop `tsc --noEmit` 与 diff 检查均通过。
- 直接启动 Vitest 曾因 sandbox 临时目录 rename 的 EPERM 在收集阶段失败（0 个测试）；通过 pnpm 重试后正常执行，未将该次失败当成产品缺陷。
- Flow 跨 Workspace 草稿丢失仍是上文记录的既有问题，本次提交不声称已解决。
