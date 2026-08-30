# ESLint warning cleanup / lint hardening

日期：2026-08-30。审计基准为 `zyqzyq/Unfour main` 的
`7392f741db300174bf2214af2dfd57ca739be5d8`，GitHub main commit 与本地 HEAD 已核对一致。
工作区起始无未提交改动。最先执行了完整 `pnpm lint`，随后用同一配置的 JSON formatter 保存逐项基线。
SSH 方式的 `git ls-remote` 被 Windows signal-pipe 权限阻止，改用 GitHub connector 的 main commit 查询核验，未变更远端。

## Baseline

实际结果：**0 errors / 89 warnings，56 个有告警文件**。数字来自本次执行，不引用历史报告。

| Rule | 类型 | 修改前 | 修改后 |
| --- | --- | ---: | ---: |
| react-hooks/exhaustive-deps | Hooks dependency | 8 | 0 |
| react-hooks/refs | Hooks render-time ref | 11 | 0 |
| react-hooks/set-state-in-effect | Hooks state/effect | 7 | 0 |
| react-hooks/immutability | Hooks callback declaration order | 2 | 0 |
| react-refresh/only-export-components | Fast Refresh mixed exports | 11 | 0 |
| complexity | Structural complexity | 19 | 10 |
| max-lines | Structural file length | 3 | 2 |
| max-lines-per-function | Structural function length | 28 | 24 |

业务/生命周期类：Hooks dependency 8、refs 11、state/effect 7、声明顺序 2、Fast Refresh 11。
结构类共 50，其中 test-only 为 4 条 describe callback 长度（与结构类重叠，不能再次相加）。
unused imports/variables、unnecessary eslint-disable、unsafe/overly broad typing、dead code、unreachable branch 的独立 ESLint 告警均为 **0**。
legacy compatibility 没有独立规则告警；browser-mocks 的长度/复杂度告警中包含仍被调用的兼容命令。

### 修改前完整 warning 清单

每行计数为 1，位置固定于基准提交；severity 全为 warning。Hook refs/state 警告消息里的“Error:”不代表 ESLint severity 为 error。

| # | Rule | 文件 | 行:列 | 类型 | 数量 | 消息摘要 |
| ---: | --- | --- | --- | --- | ---: | --- |
| 1 | react-hooks/refs | apps/desktop/src/features/account/AccountProvider.tsx | 61:40 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 2 | react-hooks/set-state-in-effect | apps/desktop/src/features/account/AccountProvider.tsx | 118:10 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 3 | react-refresh/only-export-components | apps/desktop/src/features/account/AccountSection.tsx | 80:14 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 4 | react-hooks/set-state-in-effect | apps/desktop/src/features/cloud-sync/CloudSyncProvider.tsx | 67:26 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 5 | react-hooks/set-state-in-effect | apps/desktop/src/features/cloud-sync/CloudSyncProvider.tsx | 76:5 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 6 | react-refresh/only-export-components | apps/desktop/src/features/cloud-sync/CloudSyncSection.tsx | 82:14 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 7 | react-hooks/set-state-in-effect | apps/desktop/src/features/cloud-sync/CloudWorkspaceDialog.tsx | 47:56 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 8 | react-hooks/set-state-in-effect | apps/desktop/src/features/cloud-sync/SyncConflictList.tsx | 56:26 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 9 | react-hooks/exhaustive-deps | apps/desktop/src/features/cloud-sync/SyncConflictList.tsx | 56:37 | Hooks dependency | 1 | React Hook useEffect has a missing dependency: 'load'. Either include it or remove the dependency array. |
| 10 | react-hooks/set-state-in-effect | apps/desktop/src/features/cloud-sync/WorkspaceSyncDialog.tsx | 41:21 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 11 | complexity | apps/desktop/src/features/cloud-sync/syncViewModel.ts | 9:8 | Structural complexity | 1 | Function 'getCloudSyncViewState' has a complexity of 16. Maximum allowed is 15. |
| 12 | react-hooks/refs | apps/desktop/src/features/update/UpdateProvider.tsx | 25:3 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 13 | react-refresh/only-export-components | apps/desktop/src/features/update/UpdatesSection.tsx | 53:14 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 14 | max-lines-per-function | packages/api-client/src/ApiClientPage.tsx | 18:8 | Structural function length | 1 | Function 'ApiClientPage' has too many lines (364). Maximum allowed is 300. |
| 15 | max-lines-per-function | packages/api-client/src/components/ApiCollectionTree.test.tsx | 191:31 | Structural function length | 1 | Arrow function has too many lines (285). Maximum allowed is 250. |
| 16 | max-lines-per-function | packages/api-client/src/components/ApiCollectionTree.tsx | 57:8 | Structural function length | 1 | Function 'ApiCollectionTree' has too many lines (673). Maximum allowed is 300. |
| 17 | max-lines | packages/api-client/src/components/ApiCollectionTree.tsx | 623:1 | Structural file length | 1 | File has too many lines (727). Maximum allowed is 600. |
| 18 | complexity | packages/api-client/src/components/ResponseTabs.tsx | 22:8 | Structural complexity | 1 | Function 'ResponseTabs' has a complexity of 56. Maximum allowed is 50. |
| 19 | react-refresh/only-export-components | packages/api-client/src/components/api-collection-tree-helpers.tsx | 11:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 20 | react-refresh/only-export-components | packages/api-client/src/components/api-collection-tree-helpers.tsx | 22:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 21 | react-refresh/only-export-components | packages/api-client/src/components/response-tab-views.tsx | 231:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 22 | react-refresh/only-export-components | packages/api-client/src/components/response-tab-views.tsx | 361:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 23 | react-refresh/only-export-components | packages/api-client/src/components/response-tab-views.tsx | 369:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 24 | react-refresh/only-export-components | packages/api-client/src/components/response-tab-views.tsx | 384:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 25 | max-lines-per-function | packages/api-client/src/hooks/useApiRequestTabs.ts | 38:8 | Structural function length | 1 | Function 'useApiRequestTabs' has too many lines (167). Maximum allowed is 150. |
| 26 | complexity | packages/api-client/src/hooks/useApiRequestTabs.ts | 275:1 | Structural complexity | 1 | Function 'applyGeneratedHeaders' has a complexity of 17. Maximum allowed is 15. |
| 27 | complexity | packages/api-client/src/model/request-tab-state.ts | 42:8 | Structural complexity | 1 | Function 'deriveTabResponseState' has a complexity of 16. Maximum allowed is 15. |
| 28 | max-lines-per-function | packages/app-shell/src/DesktopApp.tsx | 51:8 | Structural function length | 1 | Function 'DesktopApp' has too many lines (545). Maximum allowed is 300. |
| 29 | complexity | packages/app-shell/src/DesktopApp.tsx | 51:8 | Structural complexity | 1 | Function 'DesktopApp' has a complexity of 73. Maximum allowed is 50. |
| 30 | max-lines-per-function | packages/app-shell/src/components/WorkspaceDialogs.tsx | 15:8 | Structural function length | 1 | Function 'WorkspaceDialogs' has too many lines (325). Maximum allowed is 300. |
| 31 | max-lines-per-function | packages/app-shell/src/components/WorkspaceMenu.test.tsx | 49:27 | Structural function length | 1 | Arrow function has too many lines (283). Maximum allowed is 250. |
| 32 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/api-collections.ts | 16:8 | Structural function length | 1 | Function 'handleApiCollectionMock' has too many lines (215). Maximum allowed is 150. |
| 33 | complexity | packages/command-client/src/tauri/browser-mocks/api-collections.ts | 16:8 | Structural complexity | 1 | Function 'handleApiCollectionMock' has a complexity of 95. Maximum allowed is 15. |
| 34 | complexity | packages/command-client/src/tauri/browser-mocks/api-environments.ts | 10:8 | Structural complexity | 1 | Function 'handleApiEnvironmentMock' has a complexity of 31. Maximum allowed is 15. |
| 35 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/api-requests.ts | 21:8 | Structural function length | 1 | Async function 'handleApiRequestMock' has too many lines (231). Maximum allowed is 150. |
| 36 | complexity | packages/command-client/src/tauri/browser-mocks/api-requests.ts | 21:8 | Structural complexity | 1 | Async function 'handleApiRequestMock' has a complexity of 62. Maximum allowed is 15. |
| 37 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/database.ts | 21:8 | Structural function length | 1 | Function 'handleDatabaseMock' has too many lines (398). Maximum allowed is 150. |
| 38 | complexity | packages/command-client/src/tauri/browser-mocks/database.ts | 21:8 | Structural complexity | 1 | Function 'handleDatabaseMock' has a complexity of 111. Maximum allowed is 15. |
| 39 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/ssh.ts | 28:8 | Structural function length | 1 | Function 'handleSshMock' has too many lines (353). Maximum allowed is 150. |
| 40 | complexity | packages/command-client/src/tauri/browser-mocks/ssh.ts | 28:8 | Structural complexity | 1 | Function 'handleSshMock' has a complexity of 91. Maximum allowed is 15. |
| 41 | complexity | packages/command-client/src/tauri/browser-mocks/ssh.ts | 403:1 | Structural complexity | 1 | Function 'acceptMockSshInput' has a complexity of 22. Maximum allowed is 15. |
| 42 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/workspace.ts | 14:8 | Structural function length | 1 | Function 'handleWorkspaceMock' has too many lines (377). Maximum allowed is 150. |
| 43 | complexity | packages/command-client/src/tauri/browser-mocks/workspace.ts | 14:8 | Structural complexity | 1 | Function 'handleWorkspaceMock' has a complexity of 169. Maximum allowed is 15. |
| 44 | max-lines-per-function | packages/database/src/DatabasePage.tsx | 49:8 | Structural function length | 1 | Function 'DatabasePage' has too many lines (608). Maximum allowed is 300. |
| 45 | complexity | packages/database/src/DatabasePage.tsx | 49:8 | Structural complexity | 1 | Function 'DatabasePage' has a complexity of 62. Maximum allowed is 50. |
| 46 | react-hooks/exhaustive-deps | packages/database/src/DatabasePage.tsx | 174:6 | Hooks dependency | 1 | React Hook useMemo has an unnecessary dependency: 'activeQueryTab.connectionId'. Either exclude it or remove the dependency array. |
| 47 | react-hooks/exhaustive-deps | packages/database/src/DatabasePage.tsx | 198:6 | Hooks dependency | 1 | React Hook useEffect has a missing dependency: 'activeTab'. Either include it or remove the dependency array. |
| 48 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 223:23 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 49 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 224:5 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 50 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 225:5 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 51 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 230:39 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 52 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 231:5 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 53 | react-hooks/exhaustive-deps | packages/database/src/DatabasePage.tsx | 263:6 | Hooks dependency | 1 | React Hook useEffect has a missing dependency: 'setConnectionState'. Either include it or remove the dependency array. |
| 54 | react-hooks/exhaustive-deps | packages/database/src/DatabasePage.tsx | 274:6 | Hooks dependency | 1 | React Hook useEffect has a missing dependency: 'setConnectionState'. Either include it or remove the dependency array. |
| 55 | react-hooks/immutability | packages/database/src/DatabasePage.tsx | 298:9 | Hooks callback declaration order | 1 | Error: Cannot access variable before it is declared |
| 56 | react-hooks/exhaustive-deps | packages/database/src/DatabasePage.tsx | 347:6 | Hooks dependency | 1 | React Hook useEffect has missing dependencies: 'activeQueryTab' and 'databaseTabs'. Either include them or remove the dependency array. |
| 57 | react-hooks/refs | packages/database/src/DatabasePage.tsx | 482:3 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 58 | max-lines | packages/database/src/DatabasePage.tsx | 672:1 | Structural file length | 1 | File has too many lines (654). Maximum allowed is 600. |
| 59 | max-lines | packages/database/src/components/DatabaseConnectionTree.tsx | 686:1 | Structural file length | 1 | File has too many lines (617). Maximum allowed is 600. |
| 60 | max-lines-per-function | packages/database/src/components/SqlEditorTab.tsx | 44:8 | Structural function length | 1 | Function 'SqlEditorTab' has too many lines (448). Maximum allowed is 300. |
| 61 | max-lines-per-function | packages/database/src/components/TableDataGrid.tsx | 67:8 | Structural function length | 1 | Function 'TableDataGrid' has too many lines (394). Maximum allowed is 300. |
| 62 | max-lines-per-function | packages/database/src/hooks/useDatabaseQueryWorkspaceActions.ts | 40:8 | Structural function length | 1 | Function 'useDatabaseQueryWorkspaceActions' has too many lines (202). Maximum allowed is 150. |
| 63 | max-lines-per-function | packages/database/src/hooks/useDatabaseSqlRunner.ts | 15:8 | Structural function length | 1 | Function 'useDatabaseSqlRunner' has too many lines (267). Maximum allowed is 150. |
| 64 | max-lines-per-function | packages/database/src/hooks/useDatabaseTableBrowse.ts | 17:8 | Structural function length | 1 | Function 'useDatabaseTableBrowse' has too many lines (186). Maximum allowed is 150. |
| 65 | max-lines-per-function | packages/database/src/hooks/useDatabaseWorkspaceController.ts | 81:8 | Structural function length | 1 | Function 'useDatabaseWorkspaceController' has too many lines (289). Maximum allowed is 150. |
| 66 | react-hooks/exhaustive-deps | packages/database/src/hooks/useDatabaseWorkspaceController.ts | 205:6 | Hooks dependency | 1 | React Hook useEffect has missing dependencies: 'saveMutation', 'setEditorOpen', 'setForm', 'setPassword', and 'setTestResult'. Either include them or remove the dependency array. If 'setPassword' changes too often, find the parent component that defines it and wrap that definition in useCallback. |
| 67 | complexity | packages/database/src/model/database-query-context.ts | 4:8 | Structural complexity | 1 | Function 'normalizeQueryContext' has a complexity of 17. Maximum allowed is 15. |
| 68 | max-lines-per-function | packages/database/src/model/database-tab-state.ts | 148:61 | Structural function length | 1 | Arrow function has too many lines (159). Maximum allowed is 150. |
| 69 | complexity | packages/database/src/model/sql-statements.ts | 16:8 | Structural complexity | 1 | Function 'splitSqlStatements' has a complexity of 25. Maximum allowed is 15. |
| 70 | complexity | packages/database/src/result-utils.ts | 171:1 | Structural complexity | 1 | Function 'categorizeDatabaseError' has a complexity of 19. Maximum allowed is 15. |
| 71 | max-lines-per-function | packages/ssh-terminal/src/TerminalPage.tsx | 42:8 | Structural function length | 1 | Function 'SshConnectionsPage' has too many lines (558). Maximum allowed is 300. |
| 72 | max-lines-per-function | packages/ssh-terminal/src/components/SftpPanel.tsx | 39:8 | Structural function length | 1 | Function 'SftpPanel' has too many lines (427). Maximum allowed is 300. |
| 73 | react-hooks/exhaustive-deps | packages/ssh-terminal/src/components/SftpPanel.tsx | 111:9 | Hooks dependency | 1 | The 'entries' logical expression could make the dependencies of useMemo Hook (at line 115) change on every render. To fix this, wrap the initialization of 'entries' in its own useMemo() Hook. |
| 74 | max-lines-per-function | packages/ssh-terminal/src/components/SshConnectionTree.tsx | 51:8 | Structural function length | 1 | Function 'SshConnectionTree' has too many lines (399). Maximum allowed is 300. |
| 75 | react-hooks/refs | packages/ssh-terminal/src/components/TaskEditor.tsx | 51:3 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 76 | react-hooks/immutability | packages/ssh-terminal/src/components/TaskRunPanel.tsx | 56:45 | Hooks callback declaration order | 1 | Error: Cannot access variable before it is declared |
| 77 | max-lines-per-function | packages/ssh-terminal/src/components/TerminalPane.output.test.tsx | 52:43 | Structural function length | 1 | Arrow function has too many lines (519). Maximum allowed is 250. |
| 78 | max-lines-per-function | packages/ssh-terminal/src/components/TerminalPane.tsx | 21:8 | Structural function length | 1 | Function 'TerminalPane' has too many lines (400). Maximum allowed is 300. |
| 79 | react-hooks/refs | packages/ssh-terminal/src/hooks/useSftpNativeDragDrop.ts | 32:3 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 80 | complexity | packages/ssh-terminal/src/hooks/useSftpNativeDragDrop.ts | 48:64 | Structural complexity | 1 | Async arrow function has a complexity of 18. Maximum allowed is 15. |
| 81 | max-lines-per-function | packages/ssh-terminal/src/hooks/useSshTaskRunSession.ts | 36:8 | Structural function length | 1 | Function 'useSshTaskRunSession' has too many lines (318). Maximum allowed is 150. |
| 82 | complexity | packages/ssh-terminal/src/model/errors.ts | 3:8 | Structural complexity | 1 | Function 'formatTerminalError' has a complexity of 28. Maximum allowed is 15. |
| 83 | complexity | packages/ssh-terminal/src/model/task-run-transcript.ts | 12:8 | Structural complexity | 1 | Function 'buildTaskRunTranscript' has a complexity of 37. Maximum allowed is 15. |
| 84 | react-refresh/only-export-components | packages/ui/src/feedback.tsx | 238:17 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 85 | react-refresh/only-export-components | packages/ui/src/menus.tsx | 37:14 | Fast Refresh mixed exports | 1 | Fast refresh only works when a file only exports components. Use a new file to share constants or functions between components. |
| 86 | react-hooks/refs | packages/ui/src/tabs.tsx | 170:21 | Hooks render-time ref | 1 | Error: Cannot access refs during render |
| 87 | react-hooks/set-state-in-effect | packages/ui/src/theme.tsx | 47:5 | Hooks state/effect | 1 | Error: Calling setState synchronously within an effect can trigger cascading renders |
| 88 | max-lines-per-function | packages/ui/src/tree-view.test.tsx | 29:22 | Structural function length | 1 | Arrow function has too many lines (253). Maximum allowed is 250. |
| 89 | max-lines-per-function | packages/ui/src/tree-view.tsx | 19:8 | Structural function length | 1 | Function 'TreeView' has too many lines (351). Maximum allowed is 300. |

## Changes

- **真实缺陷**：Tabs 拖拽视觉读取 ref 不会主动重绘，改用 state；TreeView 的 joined defaults key 对 `["a|b"]` 和 `["a", "b"]` 碰撞，改用明确依赖与 applied-ID guard；Task/SFTP listeners 在卸载或异步注册完成后缺少清理，补齐清理/取消检查。
- **Database**：workspace-bound store action 使用稳定 callback；selection effect 不再漏掉同一 query tab 的 connection 变化，也不订阅 SQL/result 对象；query context 只按真实字段归一化。form reset 用 state 跟踪选择，删除 controller 中重复的清空逻辑。eager schema loader 仍只在 active/connections/connectionStates 变化时触发，cache callback 的最新值在 layout effect 中保存，不能把整个 controller 加进依赖导致失败后重复请求。
- **Cloud Sync**：冲突列表按 workspace、详情按 detail target 隔离状态；云列表按 dialog 打开读取，binding/status 变化只派生过滤，不重发请求。请求 generation 防止旧结果覆盖新界面，同一次 StrictMode setup/cleanup/setup 合并正在进行的读取。provider 保持 15s 轮询，不增加自动 sync 操作，撤权/卸载让旧 status 请求失效。
- **SSH / updater / settings**：Task draft、SFTP destination、updater install state 在 commit 后更新 ref；resize、拖拽和 late native listener 在卸载时清理。布局保存直接依赖 React Query 的稳定 mutate，保持 350ms debounce。系统主题通过 useSyncExternalStore 订阅，取消同步 effect 内重复 setState。
- **Fast Refresh**：组件与 settings descriptors、context/hooks、cookie/redaction helpers 分开，公共包导出保留，不加规则例外。展示组件、类型和纯函数只在原所属包内提取。
- **dead/redundant**：没有批量删除用途未知的代码。删除 Database controller 重复的 form/password reset；删除错误分类里已被 `denied` / `connect` 覆盖的 `access denied` / `connection` 条件；删除无用 joined defaults key。未通过 underscore、void 或 ignored pattern 隐藏 unused。
- **复杂度/测试结构**：同义错误匹配表达为数据列表，提取 body content-type、catalog lookup、sync progress、响应错误面板；四个过长 describe 按行为分组并保留原 fixture 和全部原有测试。

### Suppression 审查

删除 **10 条 disable 指令**（其中 1 处为成对 block）：feedback 4、theme 1、layout persistence 1、TreeView 1、Database 2、SSH connection selection 1。
没有 unused-disable 基线告警；这些指令是在结构修正后变得不再需要。新增仅 **3 条单行 suppression**，都附具体原因：

| 文件/位置 | Rule | 保留原因 |
| --- | --- | --- |
| AccountProvider.tsx:62 | react-hooks/refs | createAccountRefreshController 仅保存 getState callback；构造期间从不读取 ref。调用只发生在异步失败处理，是已核对的分析器误报。 |
| AccountProvider.tsx:120 | react-hooks/set-state-in-effect | 初次外部 account refresh 必须在 await 前显示 refreshing；不为消警延迟请求或改变 loading 生命周期。 |
| CloudSyncProvider.tsx:70 | react-hooks/set-state-in-effect | 外部 status refresh 同步设置 pending，并在 capability 失效时清除外部缓存；保留原轮询/加载语义。 |

原有 WorkspaceEnvironmentEditor 的 modeRevision selection boundary、Task hydration、Database 外部 cache/history 同步等局部例外经核查保留。
直接把所有环境/草稿数据加入 selection-reset effect 会覆盖未保存输入，不属于安全的 warning cleanup。
遗留命令 `api_send_request`、API environments 仍在 Tauri/command-client/command-bus 注册或包装；workspace-local 是文档明确保留的兼容边界，未删除。

## Verification

| 命令 | 结果 |
| --- | --- |
| 初始完整 `pnpm lint` | PASS (exit 0)，0 errors / 89 warnings |
| 最终完整 `pnpm lint` | PASS (exit 0)，**0 errors / 36 warnings**；不是 zero-warning gate |
| `pnpm exec eslint . --format json --output-file ...` | 基线/中间/最终清单，与完整 lint 同配置、同范围 |
| 最终完整 `pnpm test` | PASS，**117 files / 591 tests**；保留原有 546 项，净新增 45 项 |
| 聚焦生命周期测试 | PASS，Cloud Sync、updater、Database、SFTP/Task drag、Tabs/theme、layout debounce、TaskRunPanel 清理及 API response |
| 最终完整 `pnpm build` | PASS，tsc + Vite，2390 modules；仍有 >500KB chunk 提示（非 ESLint warning） |
| `pnpm run check:large-files` | PASS，0 blocking；既有结构问题仍按工具输出保留 |
| `git diff --check` | PASS |

测试重点是调用次数/取消/最新状态/草稿保留，而非 lint 字符串或实现镜像。
初次 test/build 被 Windows 沙箱拒绝 esbuild 子进程（spawn EPERM）；通过允许的沙箱外执行重跑成功，没有跳过测试或改测试配置。
未运行 Rust tests/checks：没有修改 Rust、Tauri schema、command-bus contract 或后端调用链。
未运行真实 Tauri/SSH/数据库/Cloud 双设备/打包 updater 手动验收或 E2E：本次为前端 lint 清理和确定性单元/组件回归，不能把 mock 通过当作真实服务验证。
SFTP cleanup 阻止后续上传和卸载后 UI callback，不声称能取消已交给后端执行的那一个传输。

## Remaining

**未达到 0 warnings**。89 → 36，减少 53。剩余 24 function-length、10 complexity、2 file-length，全部保持可见，未新增 suppression 或配置例外。
下表逐条记录当前行号、消息、保留原因。不是声称这些代码永远不可重构；在本轮禁止大规模业务重构的约束下，应分别做有行为验收的后续工作。

| # | Rule | 文件 | 行:列 | 数量 | 当前信号 | 本轮保留原因/后续边界 |
| ---: | --- | --- | --- | ---: | --- | --- |
| 1 | max-lines-per-function | packages/api-client/src/ApiClientPage.tsx | 18:8 | 1 | Function 'ApiClientPage' has too many lines (364). Maximum allowed is 300. | 请求编辑、响应与 shell slot 组合集中；下一步提取展示子组件，不能改变 Send/Save/history 时序。 |
| 2 | max-lines-per-function | packages/api-client/src/components/ApiCollectionTree.tsx | 57:8 | 1 | Function 'ApiCollectionTree' has too many lines (673). Maximum allowed is 300. | 集合/文件夹/请求菜单、拖拽和编辑状态混合；需单独分离 tree model 与 mutations 并验证移动/重命名，不能为长度改变资源操作。 |
| 3 | max-lines | packages/api-client/src/components/ApiCollectionTree.tsx | 623:1 | 1 | File has too many lines (727). Maximum allowed is 600. | 集合/文件夹/请求菜单、拖拽和编辑状态混合；需单独分离 tree model 与 mutations 并验证移动/重命名，不能为长度改变资源操作。 |
| 4 | max-lines-per-function | packages/api-client/src/hooks/useApiRequestTabs.ts | 38:8 | 1 | Function 'useApiRequestTabs' has too many lines (167). Maximum allowed is 150. | 查询、Send/Save mutation、tab actions 共用 workspace 闭包；仅提取了纯 header 规则，进一步拆分需独立验证并发 Send 与保存基线。 |
| 5 | max-lines-per-function | packages/app-shell/src/DesktopApp.tsx | 51:8 | 1 | Function 'DesktopApp' has too many lines (545). Maximum allowed is 300. | 架构允许的桌面 composition root，slots/导航较多；需在 app-shell 内拆分，不得把业务移入 shell 或改变模块挂载。 |
| 6 | complexity | packages/app-shell/src/DesktopApp.tsx | 51:8 | 1 | Function 'DesktopApp' has a complexity of 73. Maximum allowed is 50. | 架构允许的桌面 composition root，slots/导航较多；需在 app-shell 内拆分，不得把业务移入 shell 或改变模块挂载。 |
| 7 | max-lines-per-function | packages/app-shell/src/components/WorkspaceDialogs.tsx | 15:8 | 1 | Function 'WorkspaceDialogs' has too many lines (325). Maximum allowed is 300. | 创建/删除/设置对话框共用输入与异步确认；可另行按对话框拆分，需保护失败后保留输入与 workspace 切换。 |
| 8 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/api-collections.ts | 16:8 | 1 | Function 'handleApiCollectionMock' has too many lines (215). Maximum allowed is 150. | 浏览器命令分发器聚合集合/文件夹 CRUD；不是死代码，需按命令族拆分并保留 mock 契约。 |
| 9 | complexity | packages/command-client/src/tauri/browser-mocks/api-collections.ts | 16:8 | 1 | Function 'handleApiCollectionMock' has a complexity of 95. Maximum allowed is 15. | 浏览器命令分发器聚合集合/文件夹 CRUD；不是死代码，需按命令族拆分并保留 mock 契约。 |
| 10 | complexity | packages/command-client/src/tauri/browser-mocks/api-environments.ts | 10:8 | 1 | Function 'handleApiEnvironmentMock' has a complexity of 31. Maximum allowed is 15. | 旧 API environment adapter 仍被 command-client 和 Rust 注册引用；不能按 legacy 名称删除，需单独迁移兼容接口。 |
| 11 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/api-requests.ts | 21:8 | 1 | Async function 'handleApiRequestMock' has too many lines (231). Maximum allowed is 150. | 保存/历史/发送及 v1/v2 返回形状共用分发器；兼容命令仍存在，拆分需逐条契约验证。 |
| 12 | complexity | packages/command-client/src/tauri/browser-mocks/api-requests.ts | 21:8 | 1 | Async function 'handleApiRequestMock' has a complexity of 62. Maximum allowed is 15. | 保存/历史/发送及 v1/v2 返回形状共用分发器；兼容命令仍存在，拆分需逐条契约验证。 |
| 13 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/database.ts | 21:8 | 1 | Function 'handleDatabaseMock' has too many lines (398). Maximum allowed is 150. | 浏览器 DB CRUD、schema、SQL/table mocks 集中；需独立拆分 handlers 并验证确认/只读语义，不能视作测试专用废代码删除。 |
| 14 | complexity | packages/command-client/src/tauri/browser-mocks/database.ts | 21:8 | 1 | Function 'handleDatabaseMock' has a complexity of 111. Maximum allowed is 15. | 浏览器 DB CRUD、schema、SQL/table mocks 集中；需独立拆分 handlers 并验证确认/只读语义，不能视作测试专用废代码删除。 |
| 15 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/ssh.ts | 28:8 | 1 | Function 'handleSshMock' has too many lines (353). Maximum allowed is 150. | 会话/host-key/历史命令分发器和 PTY 输入模拟状态机；需分别验证命令语义、退格/控制序列后拆分。 |
| 16 | complexity | packages/command-client/src/tauri/browser-mocks/ssh.ts | 28:8 | 1 | Function 'handleSshMock' has a complexity of 91. Maximum allowed is 15. | 会话/host-key/历史命令分发器和 PTY 输入模拟状态机；需分别验证命令语义、退格/控制序列后拆分。 |
| 17 | complexity | packages/command-client/src/tauri/browser-mocks/ssh.ts | 403:1 | 1 | Function 'acceptMockSshInput' has a complexity of 22. Maximum allowed is 15. | 会话/host-key/历史命令分发器和 PTY 输入模拟状态机；需分别验证命令语义、退格/控制序列后拆分。 |
| 18 | max-lines-per-function | packages/command-client/src/tauri/browser-mocks/workspace.ts | 14:8 | 1 | Function 'handleWorkspaceMock' has too many lines (377). Maximum allowed is 150. | workspace 生命周期、变量和 environments mock 命令集中；包含活动对象/级联/解析规则，需按命令族拆分和契约测试。 |
| 19 | complexity | packages/command-client/src/tauri/browser-mocks/workspace.ts | 14:8 | 1 | Function 'handleWorkspaceMock' has a complexity of 169. Maximum allowed is 15. | workspace 生命周期、变量和 environments mock 命令集中；包含活动对象/级联/解析规则，需按命令族拆分和契约测试。 |
| 20 | max-lines-per-function | packages/database/src/DatabasePage.tsx | 47:8 | 1 | Function 'DatabasePage' has too many lines (539). Maximum allowed is 300. | 本次已提取表单与 tab 同步并降低复杂度/文件长度，剩余为 sidebar/workspace/dialog 组合；继续拆分应另立布局组合任务。 |
| 21 | max-lines | packages/database/src/components/DatabaseConnectionTree.tsx | 686:1 | 1 | File has too many lines (617). Maximum allowed is 600. | 连接/schema 树及菜单集中；需要树节点模型与菜单责任拆分，不应为了 17 行超额改动 lazy schema 行为。 |
| 22 | max-lines-per-function | packages/database/src/components/SqlEditorTab.tsx | 44:8 | 1 | Function 'SqlEditorTab' has too many lines (448). Maximum allowed is 300. | SQL 编辑器、快捷键和结果工具栏组合较长；需保持 Monaco selection/run-all/confirmation 行为后独立拆分。 |
| 23 | max-lines-per-function | packages/database/src/components/TableDataGrid.tsx | 67:8 | 1 | Function 'TableDataGrid' has too many lines (394). Maximum allowed is 300. | 表格编辑、选择、菜单和键盘交互共用状态；需专项回归编辑冲突与 selection，避免本轮大规模重构。 |
| 24 | max-lines-per-function | packages/database/src/hooks/useDatabaseQueryWorkspaceActions.ts | 40:8 | 1 | Function 'useDatabaseQueryWorkspaceActions' has too many lines (202). Maximum allowed is 150. | query/table/history 操作共用 tab 状态；需按动作族拆分，验证选中上下文与保存 SQL 的归属。 |
| 25 | max-lines-per-function | packages/database/src/hooks/useDatabaseSqlRunner.ts | 15:8 | 1 | Function 'useDatabaseSqlRunner' has too many lines (267). Maximum allowed is 150. | 执行/停止/确认/批次结果和历史时序耦合；长度规则保留，另行拆分状态机并验证取消与失败。 |
| 26 | max-lines-per-function | packages/database/src/hooks/useDatabaseTableBrowse.ts | 17:8 | 1 | Function 'useDatabaseTableBrowse' has too many lines (186). Maximum allowed is 150. | 分页/排序/过滤/变更提交共用取消和请求标识；不能仅为长度拆散竞态保护。 |
| 27 | max-lines-per-function | packages/database/src/hooks/useDatabaseWorkspaceController.ts | 81:8 | 1 | Function 'useDatabaseWorkspaceController' has too many lines (286). Maximum allowed is 150. | 组合多个 controller 和 workspace actions；本次移除了重复表单重置，进一步拆分需保持回调和 mutations 生命周期。 |
| 28 | max-lines-per-function | packages/database/src/model/database-tab-state.ts | 148:61 | 1 | Arrow function has too many lines (159). Maximum allowed is 150. | Zustand initializer 定义完整 tab actions；需按 store action 责任提取，保护默认 tab/删除/重排不变量。 |
| 29 | complexity | packages/database/src/model/sql-statements.ts | 16:8 | 1 | Function 'splitSqlStatements' has a complexity of 25. Maximum allowed is 15. | SQL scanner 的引号、注释和分隔符状态分支；不作机械条件改写，需独立扩展 parser corpus 后拆分。 |
| 30 | max-lines-per-function | packages/ssh-terminal/src/TerminalPage.tsx | 42:8 | 1 | Function 'SshConnectionsPage' has too many lines (558). Maximum allowed is 300. | 连接、session、host-key、workspace slot 编排；保留可见长度信号，后续需验证真实 session/重连生命周期。 |
| 31 | max-lines-per-function | packages/ssh-terminal/src/components/SftpPanel.tsx | 39:8 | 1 | Function 'SftpPanel' has too many lines (427). Maximum allowed is 300. | 远端导航和传输/覆盖确认组合；仅稳定 entries identity，进一步拆分需验证上传/下载/取消与路径状态。 |
| 32 | max-lines-per-function | packages/ssh-terminal/src/components/SshConnectionTree.tsx | 51:8 | 1 | Function 'SshConnectionTree' has too many lines (399). Maximum allowed is 300. | 连接节点与对象菜单共用上下文；可按 tree model/actions 拆分，需保留连接/信任/删除操作。 |
| 33 | max-lines-per-function | packages/ssh-terminal/src/components/TerminalPane.tsx | 21:8 | 1 | Function 'TerminalPane' has too many lines (400). Maximum allowed is 300. | xterm、addons、PTY/event/history/clipboard 生命周期耦合；不能为长度拆分初始化和 disposal 边界。 |
| 34 | max-lines-per-function | packages/ssh-terminal/src/hooks/useSshTaskRunSession.ts | 36:8 | 1 | Function 'useSshTaskRunSession' has too many lines (318). Maximum allowed is 150. | task run、取消、events、日志和 environment hydration 共用状态；需专项执行/取消/卸载测试后拆分。 |
| 35 | complexity | packages/ssh-terminal/src/model/task-run-transcript.ts | 12:8 | 1 | Function 'buildTaskRunTranscript' has a complexity of 37. Maximum allowed is 15. | 有序 step/output/transfer 事件合成状态机；需针对乱序/缺失/失败 transfer 扩充 corpus 后拆分，保留规则信号。 |
| 36 | max-lines-per-function | packages/ui/src/tree-view.tsx | 19:8 | 1 | Function 'TreeView' has too many lines (350). Maximum allowed is 300. | 键盘/typeahead/native+pointer DnD 共用焦点状态；修复 defaults 依赖不扩张为整棵树重构。 |

独立 rule 建议：本轮**不建议关闭或放宽现有规则**。优先按命令族拆 browser mock dispatcher，再拆展示/菜单组件；SQL scanner、transcript、session hooks 单独验证。
如团队日后评估复杂度与文件长度阈值，应与结构改造分开评审，而非针对这些文件加 ignore；完成结构债务后再考虑 CI `--max-warnings=0`。

## 修改文件与人工审阅

| 文件 | 目的 |
| --- | --- |
| apps/desktop/src/features/account/AccountProvider.tsx | 记录 controller factory 的 refs 误报与初次外部 refresh pending-state 的两处单行例外；不改 account 生命周期。 |
| apps/desktop/src/features/account/AccountSection.tsx | 组件/翻译 Label 与 settings descriptor 分离。 |
| apps/desktop/src/features/account/index.ts | settings descriptor 移至 feature barrel，组件文件只导出组件。 |
| apps/desktop/src/features/cloud-sync/CloudSyncProvider.test.tsx | 增加轮询次数、普通 rerender、卸载/撤权后的零后续请求测试。 |
| apps/desktop/src/features/cloud-sync/CloudSyncProvider.tsx | 失效请求提前退出、卸载失效化、撤权时在子树渲染前清除 dialogs；保留轮询周期。 |
| apps/desktop/src/features/cloud-sync/CloudSyncSection.tsx | 组件与 settings descriptor 分离。 |
| apps/desktop/src/features/cloud-sync/CloudWorkspaceDialog.tsx | 打开时加载、按本地 binding 派生过滤、过期请求隔离与 StrictMode 同次读取合并。 |
| apps/desktop/src/features/cloud-sync/SyncConflictList.tsx | 按 workspace 隔离状态、稳定 load、合并挂载读取、卸载后禁止旧 mutation 触发刷新。 |
| apps/desktop/src/features/cloud-sync/WorkspaceSyncDialog.tsx | 用 target key 隔离 diagnostics、error、busy、remote confirmation。 |
| apps/desktop/src/features/cloud-sync/index.ts | settings descriptor 移至 feature barrel，组件文件只导出组件。 |
| apps/desktop/src/features/cloud-sync/syncUi.test.tsx | 增加 StrictMode、关闭重开、binding 更新、workspace 切换及旧确认隔离回归。 |
| apps/desktop/src/features/cloud-sync/syncViewModel.test.ts | 保护 pause/error 与 pending/initial sync 的显示优先级。 |
| apps/desktop/src/features/cloud-sync/syncViewModel.ts | 提取 sync-in-progress 判断，保持 pause/error 优先级。 |
| apps/desktop/src/features/update/UpdateProvider.test.tsx | 覆盖最新版本安装、重复安装防护、进度 timer 清理与单次延迟自动检查。 |
| apps/desktop/src/features/update/UpdateProvider.tsx | 在 layout effect 同步 install 所读的已提交状态；不改变自动检查延迟。 |
| apps/desktop/src/features/update/UpdatesSection.tsx | 组件与 settings descriptor 分离。 |
| apps/desktop/src/features/update/index.ts | settings descriptor 移至 feature barrel，组件文件只导出组件。 |
| packages/api-client/src/components/ApiCollectionTree.test.tsx | 将拖拽用例分组；保留全部断言及全局 fixture 清理。 |
| packages/api-client/src/components/ApiCollectionTree.tsx | 仅调整 SidebarEmpty 导入来源。 |
| packages/api-client/src/components/ApiTreeLabels.tsx | 容纳 MethodMeta/SidebarEmpty 展示组件。 |
| packages/api-client/src/components/ResponseTabs.test.tsx | 保留全部原有响应/脚本/脱敏测试，增加 network/timeout/failure 的显式 retry 测试。 |
| packages/api-client/src/components/ResponseTabs.tsx | 错误面板与 post-script 状态标签提取到展示组件，调整 helper 导入。 |
| packages/api-client/src/components/api-collection-tree-helpers.tsx | 树节点 factory 与展示组件分开，消除 mixed exports。 |
| packages/api-client/src/components/response-tab-helpers.ts | 迁移 cookie parsing、key-value redaction 与状态 label helpers；不改算法。 |
| packages/api-client/src/components/response-tab-views.tsx | 只导出组件，承接错误面板和 post-script badge。 |
| packages/api-client/src/hooks/useApiRequestTabs.test.ts | 保护显式 content-type 优先与 disabled form 字段语义。 |
| packages/api-client/src/hooks/useApiRequestTabs.ts | 分离 body content-type 推导与 Auth header 合成。 |
| packages/api-client/src/model/request-tab-state.ts | 把网络错误同义匹配表达为列表，保留 timeout 优先。 |
| packages/api-client/src/model/request-tabs.test.ts | 增加网络同义项、timeout 与通用 failure 回归。 |
| packages/app-shell/src/components/WorkspaceMenu.test.tsx | 把 extension 用例独立分组，保留现有测试。 |
| packages/app-shell/src/components/useLayoutPersistence.test.tsx | 验证 350ms debounce 不因 mutation rerender 重置、卸载取消保存。 |
| packages/app-shell/src/components/useLayoutPersistence.ts | 直接依赖稳定 mutate，移除 render-time ref 与 suppression。 |
| packages/database/src/DatabasePage.tsx | 稳定 workspace callback；只按有效 tab 字段同步；提取表单、types；loader/sidebar callbacks 在 commit 后同步。 |
| packages/database/src/hooks/useDatabaseConnectionForm.test.tsx | 同选择的 cache rerender 保留草稿，连接/工作区变化清除密码和旧归属。 |
| packages/database/src/hooks/useDatabaseConnectionForm.ts | 以 state 记录前次选择，在 render 重试中隔离凭据和编辑草稿，替代 render-time ref。 |
| packages/database/src/hooks/useDatabaseTabSynchronization.test.tsx | 真实 tab store 下验证 SQL 编辑不重置 selection、schema 归一化不循环。 |
| packages/database/src/hooks/useDatabaseTabSynchronization.ts | 按 tab ID/connection/table 与 catalog/schema 同步，排除 SQL/result 对象 churn。 |
| packages/database/src/hooks/useDatabaseWorkspaceController.ts | 删除与 form hook 重复的 workspace 清理，effect 仅重置稳定 save mutation。 |
| packages/database/src/model/database-connection-state.test.ts | 验证 disconnected/failed/unknown 不启用 schema。 |
| packages/database/src/model/database-connection-state.ts | 统一 schema 可加载状态判断（connecting/connected）。 |
| packages/database/src/model/database-page.ts | 承接 sidebar action 类型，保持在 Database 包内。 |
| packages/database/src/model/database-query-context.ts | 提取 catalog 查找，保留 unnamed SQLite 与显式 server catalog 区别。 |
| packages/database/src/result-utils.test.ts | 保护 permission/syntax/network/connection 优先级。 |
| packages/database/src/result-utils.ts | 错误同义项列表化，删除被 denied/connect 已覆盖的冗余条件。 |
| packages/ssh-terminal/src/TerminalPage.tsx | 用 state 记录前次连接选择，删除 refs block suppression；不改 session hooks。 |
| packages/ssh-terminal/src/components/SftpPanel.tsx | 稳定 directory entries fallback 的对象身份。 |
| packages/ssh-terminal/src/components/TaskEditor.tsx | 将拖拽生命周期交给包内 hook，移除 render-time ref。 |
| packages/ssh-terminal/src/components/TaskRunPanel.test.tsx | 验证 up/cancel/unmount 清理监听且不取消 SSH run。 |
| packages/ssh-terminal/src/components/TaskRunPanel.tsx | 具名 resize end callback，补 pointercancel 与卸载清理。 |
| packages/ssh-terminal/src/components/TerminalPane.output.test.tsx | 按输出、suggestions、输入边界、history 隔离分组，保留全部测试与相同 fixture。 |
| packages/ssh-terminal/src/hooks/useSftpNativeDragDrop.test.tsx | 验证 late unlisten、路径更新不重注册、hit-test/upload pending 时卸载不发下一条命令。 |
| packages/ssh-terminal/src/hooks/useSftpNativeDragDrop.ts | commit 后更新目标、late registration disposal、await 后取消检查、拆分上传循环。 |
| packages/ssh-terminal/src/hooks/useTaskStepDrag.test.tsx | 验证拖拽期间编辑保留、只提交一次、卸载不重排。 |
| packages/ssh-terminal/src/hooks/useTaskStepDrag.ts | 独立拖拽 hook 使用最新已提交 draft/callback，集中清理 window listeners。 |
| packages/ssh-terminal/src/model/errors.test.ts | 增加 host-key/auth/timeout 优先级回归。 |
| packages/ssh-terminal/src/model/errors.ts | SSH 错误同义项列表化，保留优先级与 fallback 脱敏。 |
| packages/ui/src/feedback-context.ts | 迁移 feedback context/hooks 与纯 helpers，避免组件文件 mixed exports；脱敏算法不变。 |
| packages/ui/src/feedback.test.ts | 更新 helper 导入，保留原有断言。 |
| packages/ui/src/feedback.tsx | 只保留 provider/toast，删除四条 Fast Refresh suppression。 |
| packages/ui/src/index.ts | 保持包级公共导出不变，改从独立 context/helper 模块导出。 |
| packages/ui/src/menus.tsx | 以命名组件包装 Radix Root，便于 Fast Refresh 正确识别。 |
| packages/ui/src/tabs.test.tsx | 验证立即显示拖拽状态、只重排一次、drop/end 清除状态。 |
| packages/ui/src/tabs.tsx | drag 视觉状态由 state 驱动，ref 只处理即时拖拽事件。 |
| packages/ui/src/theme-context.ts | 分离 theme context/types/hook，移除 circular-import suppression 理由。 |
| packages/ui/src/theme-init.ts | 更新 theme type 导入。 |
| packages/ui/src/theme-internal.ts | 更新 theme type 导入。 |
| packages/ui/src/theme.test.tsx | 验证 system/manual 切换、持久化与单一订阅清理。 |
| packages/ui/src/theme.tsx | 用 useSyncExternalStore 订阅系统主题，layout effect 只应用 DOM 属性。 |
| packages/ui/src/tree-view.test.tsx | 分离 DnD 分组；验证 join key 碰撞被修复且不重新展开手动折叠节点。 |
| packages/ui/src/tree-view.tsx | 直接依赖 defaults 数组，用已应用 ID guard 防重复；删除碰撞风险的 join key 与 suppression。 |
| docs/testing/eslint-warning-cleanup.md | 本次基线、逐条清单、验证结果、例外与剩余风险。 |

业务逻辑：**有**，仅前端生命周期/请求隔离/交互与冗余逻辑修复。新依赖：**无**。
包依赖方向、公开命令/Tauri contract、后端调用链：**未改变**。ESLint 配置：**未改变**。
跨包修改的原因是全仓 lint 告警位于不同拥有者；每处修改留在原包，未把 feature 业务搬进 app-shell/ui。
仍超推荐长度的已修改文件仅做相关小提取或导入/测试分组修正；没有为阈值进行无关大重写。

人工优先审阅：DatabasePage 与两个 synchronization/form hooks（selection/reset/schema 触发边界）、CloudSyncProvider/两个列表/WorkspaceSyncDialog（过期请求与 target 隔离）、useSftpNativeDragDrop/useTaskStepDrag/TaskRunPanel（异步 cleanup）、theme subscription，以及 AccountProvider 的两处和 CloudSyncProvider 的一处局部 suppression。
