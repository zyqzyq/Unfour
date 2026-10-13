# SSH Task 输出竞态回归 — 2026-10-13

基线为最新 `origin/main` 的 `91eb9699`（开始时本地与远端一致，工作区干净）。
修复仅涉及 SSH Task 前端 Hook、回归测试及本记录，没有新增依赖或修改公共
Hook 接口、Rust SSH 引擎、持久化 Schema、MCP、Cloud Sync 或 Lint 阈值。

## 根因与失效边界

`useSshTaskRunSession.openHistoryRun` 先读取 Task 详情，再设置 Run 选择并调用
`useSshTaskRunOutput.loadHistoryLog`。实时 IPC 事件通过 16 ms 批次进入缓存；
隐藏面板保留缓冲，并每 250 ms 检查是否可以刷新。

原实现清理成功后只重置已提交缓存。日志读取的成功、失败及 `finally` 没有
生命周期检查，订阅闭包的待刷新事件也没有被清理。因此旧日志会重新写入
缓存，旧事件会在后续批次重新出现，任意一个并发读取还会提前结束加载状态。
详情读取乱序完成时，旧 Run 选择也可能覆盖新选择。

修复采用 Workspace 生命周期 token、日志 generation、每次读取的请求标识，
以及历史选择版本号。日志处理由内部 `useSshTaskRunHistory` 管理；成功、失败、
错误反馈及加载状态更新均检查结果是否仍有效。缓存引用在写入时同步更新，
防止 React 尚未提交时再次命中旧引用。并发读取继续保留首个完成的日志；
加载状态根据当前选中 Run 的在途请求计算。

| 场景 | 处理 |
| --- | --- |
| 清理成功 | 递增日志 generation，重置原有全部历史日志缓存和对应加载记录；移除该 Task 的已提交及待刷新事件；使该 Task 未完成的历史选择失效 |
| 清理失败 | 保留日志请求、已提交输出和事件缓冲，继续报告清理错误 |
| 成功后收到新事件 | 正常接受，包括同一运行中的 Run；不永久屏蔽 Run ID，不按时间戳推断事件新旧 |
| Workspace 切换或卸载 | 使旧日志、历史选择和清理请求的前端输出更新与错误反馈失效；Workspace 切换同时重置输出缓存和 Run 选择 |
| 详情读取乱序、关闭面板 | 仅当前选择请求可以设置 Run 并发起日志读取 |
| 清理另一个 Task | 其事件保留，正在打开的其他 Task 历史选择仍可完成 |
| 切换界面后旧清理完成 | 不修改新生命周期的输出；仍准确失效发起清理的 Workspace 查询缓存 |

现有上限保持不变：待刷新事件 10,000 条、每 Run 5,000 条、实时缓存 16 个
Run、历史日志缓存 4 个 Run。传输进度合并、终态查询失效、隐藏缓冲、空日志
缓存命中及当前有效请求的错误反馈均保持原行为。

## 回归与验证

新增 33 个用例，位于 `useSshTaskRunOutput.test.tsx` 和
`useSshTaskRunSession.test.tsx`。覆盖日志读取中清理、清理期间已刷出或隐藏
缓冲的事件、成功与失败、旧日志成功与失败返回、Workspace 切换及切走再
切回、保留的旧回调、同 Run 并发读取、快速切换与乱序详情、关闭面板、卸载、
其他 Task 的选择及 React 尚未提交的缓存访问。

使用原版 main 的输出 Hook 运行新测试中的 4 个核心用例，得到预期的 FAIL：
旧日志分别重写已删除文本和空日志，旧事件在可见及隐藏面板中重新出现。
随后恢复修复版本，最终验证如下：

| 命令 | 结果 |
| --- | --- |
| `pnpm exec vitest run packages/ssh-terminal` | PASS：37 文件，218 测试 |
| `pnpm test` | PASS：176 文件，1160 测试 |
| `pnpm run lint` | PASS：0 errors，32 个已有警告；修改涉及的 Session 函数长度警告在基线已存在 |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| `pnpm run build` | PASS：保留现有大于 500 kB 的 chunk 提示 |
| `pnpm run check:large-files` | PASS：0 blocking |
| `git diff --check` | PASS |

## 验证边界

上述测试使用延迟 Promise 和受控计时器验证前端竞态，真实 SSH 与原生 IPC
链路为 **NOT VERIFIED**。事件边界以清理成功时已送达前端订阅的事件为准。
现有 IPC 事件没有 Workspace 或清理 epoch 字段；尚未送达的原生消息与后端
清理的跨端排序不由本次前端 generation 证明。本次保留该协议，避免通过
Run 黑名单或时间戳过滤误丢正常新输出。

## 后续修正：按 Task 失效历史日志

上一轮清理成功时会推进共用的日志 generation，并清空全部历史日志和在途
读取。实时事件已经按 Task 删除，但历史日志只按 Run ID 缓存，面板显示的是
`historyLogByRun[activeRun.id]`，缓存被清掉后不会自动重新请求。因此清理
Task A 时，正在查看或加载的 Task B 日志会消失且停在空白。

修正只记录每次读取所属的 Task。清理 A 时删除 A 的日志和在途请求，并只递增
A 的 generation，所以 A 的旧响应和旧缓冲事件不能写回；之后的新事件和新读取
仍然接受。B 的缓存、在途读取和实时事件不动。Workspace 切换仍重置全部缓存
并推进共用 generation，卸载仍使旧请求失效。清理失败不调用输出清理。
Session 返回值、缓存上限、Rust、IPC 和持久化格式都没有改。上文清理成功
的表格描述的是上一轮行为；当前契约以下表为准。

| 场景 | 处理 |
| --- | --- |
| 清理成功 | 只递增该 Task 的日志 generation，删除它的历史日志、在途读取和已提交/待刷新事件；使该 Task 未完成的历史选择失效 |
| 清理另一个 Task | 其已缓存日志、在途读取、事件和正在打开的历史选择都保留，并在读取完成后正常显示 |
| 清理失败 | 不修改任何 Task 的日志、事件或在途读取 |
| Workspace 切换或卸载 | 仍使全部历史日志和在途读取失效 |

| 命令 | 结果 |
| --- | --- |
| `pnpm exec vitest run packages/ssh-terminal` | PASS：37 文件，221 测试 |
| `pnpm test` | PASS：176 文件，1167 测试 |
| `pnpm run lint` | PASS：0 errors，32 个已有警告；Session 函数长度警告在基线已存在 |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| `pnpm run build` | PASS：保留现有大于 500 kB 的 chunk 提示 |
| `git diff --check` | PASS |
| `pnpm run check:large-files` | PASS：0 blocking |

真实 SSH 与原生 IPC 链路仍为 **NOT VERIFIED**。
