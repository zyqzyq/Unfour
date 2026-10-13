# 第四轮可维护性重构：SSH 任务输出生命周期

## 基线和范围选择

延续 [全仓评估](architecture-health-2026-10-12.md) 和
[第三轮报告](architecture-health-2026-10-13-round-3.md)。开始时工作区干净，fetch 后
main、HEAD、origin/main 均为 `f9dce609`。直接在 main 修改，不创建分支。

本轮重新执行全仓 ESLint 与超长文件扫描，并检查 SSH task session、输出缓存模型、
command-client channel 包装、页面消费方和现有测试。基线为 0 errors / 32 warnings，
87 个超长文件。没有重新宣称逐行审计整个仓库。

| 投入优先级 | 发现与依据 | 本轮决定 |
| --- | --- | --- |
| P1，本轮 | `useSshTaskRunSession` 混合环境表单、运行命令、输出订阅、批处理、历史读取及缓存。有效函数长度 318 行；虽已有纯缓存模型与面板测试，但没有 session hook 的直接测试。 | 先补异步生命周期保护，再将实时输出与历史缓存一起交给专用 hook。 |
| P2，下一轮 | 环境表单仍含同步 effect、可选 defaults 加载、手工环境选择和 mutation reset。纯 input/model 已分离，但自动同步与用户输入的交互仍需专门测试。 | 本轮保持这些相互关联的状态、effect 和操作原位，避免同时改变过多异步边界。 |
| P2，专项 | 历史打开和任务准备的并发请求尚没有统一的过期响应策略；loading 为一个共享布尔值。 | 保持既有行为；单独定义 workspace 切换、并发打开和清理期间迟到响应的契约，再决定是否改为最新请求优先。 |
| 保留 | MCP/Cloud Sync、安全执行边界、长集成测试和声明式 schema。 | 延续前几轮风险判断，不把文件长度直接等同设计缺陷。 |

候选 session 在基线最近 100 个仓库提交中没有修改，不能称为近期高频热点。
选择它是因为输出订阅与清理具有真实时序风险、职责边界清晰、直接保护不足，且可在不连接
远端服务的条件下建立回归证据。现有纯缓存模型继续复用，不新增缓存框架或跨包抽象。

## 实际修改

新增 `packages/ssh-terminal/src/hooks/useSshTaskRunOutput.ts`，集中拥有：

- 事件订阅及其延迟注册、dispose、定时器与 pending batch。
- 面板 active 状态和选中 run 的最新 refs，输出/历史缓存及其读取 refs。
- 历史日志读取、失败反馈、缓存命中判断和成功清理后的缓存更新。

session 通过 `loadHistoryLog(runId)`、`clearTaskOutput(taskId)` 使用这些职责，读取输出状态，
不再直接修改缓存。环境表单、task detail 获取、active run 选择和 run/cancel/clear mutation
仍归 session 编排。`useSshTaskRunSession` 的公开返回字段、页面消费方式保持一致。
订阅 effect 的代码与原实现逐字比对一致；没有将 refs、timer 与 cleanup 分散到独立生命周期。

保持以下原有语义：

- 首批等待 16 ms；隐藏 Tasks 时保留 pending，并每 250 ms 检查；切换 active 不重建订阅。
- pending 保留最新 10000 项，单 run 最多 5000 项，实时最多 16 个 run，历史最多 4 个 log。
- 最新选中 run 在缓存淘汰时受保护；同 step transfer 使用既有纯模型压缩。
- workspace 切换或卸载丢弃旧 pending、取消 timer；迟到注册立即 dispose，旧 listener 忽略新消息。
- 仅已完成的 run 事件触发对应 workspace/task 查询失效；卸载本身不取消远端任务。
- 有已提交的实时事件或已缓存历史时不再读日志；空日志和读取失败后的空结果同样命中缓存。
- 同 run 并发日志读取由第一个完成并写入的结果占据缓存，后续结果不覆盖。
- 清理成功才删该 task 的实时缓存、清空所有历史缓存；失败保留输出并报告原有错误 key。

未改公共接口、UI 组件/文案、依赖、协议、持久化格式、凭据处理或 Rust 执行链。
未修改 AGENTS.md、Lint/超长文件配置或排除项，也没有新增规则 suppression。

## 指标与实际收益

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| ESLint errors / warnings | 0 / 32 | 0 / 32 |
| complexity / max-lines-per-function warnings | 11 / 18 | 11 / 18 |
| max-lines / unused-vars / set-state-in-effect | 1 / 1 / 1 | 1 / 1 / 1 |
| session 有效函数长度 | 318 | 236 |
| session 物理行数 | 376 | 280 |
| 独立 output hook 物理行数 | 0 | 134 |
| 超长文件 | 87 | 87 |
| Critical / P0 / P1（长度分类） | 0 / 20 / 67 | 0 / 20 / 67 |
| blocking / grandfathered | 0 / 0 | 0 / 0 |
| session 直接回归测试 | 0 | 16 |

新 output hook 与测试无 Lint 告警，session 仍保留函数长度告警；没有为了让它低于 150 行
继续分散表单逻辑。两份实现总计增加 38 物理行，代价来自明确的 hook 接口与职责边界。
实际收益是 4 个输出相关 refs、3 份输出状态及异步清理逻辑集中管理，调用方只表达读取/清理意图，
并首次有公开 session 层面的时序回归测试。警告数量没有下降，不将其包装为清零成果。

## 验证证据

新增 `useSshTaskRunSession.test.tsx`，从原有公开 hook 调用路径验证行为，使用真实 QueryClient、
React hook 渲染、受控 channel Promise 和 fake timers；后端操作全部 mock，没有连接真实 SSH。
16 项覆盖批处理、隐藏/恢复、两种卸载时序、workspace 切换、StrictMode、队列/缓存上限、
active run 保护、transfer 合并、日志加载/失败/命中/并发、启动、取消及清理成功/失败。

| 检查 | 结果 |
| --- | --- |
| 原实现上的新测试首跑 | FAIL：15 通过、1 失败；测试把 StrictMode 放在 Provider 内，未触发根级重复 effect。仅修正测试为 `reactStrictMode: true`，未改业务实现。 |
| 修正测试挂载后的原实现 | PASS：16 项；然后才提取 output hook。 |
| `pnpm exec vitest run packages/ssh-terminal` | PASS：36 文件、185 项。 |
| `pnpm test` | PASS：175 文件、1127 项；上一轮为 174 文件、1111 项。 |
| `pnpm run build` | PASS：TypeScript + Vite；既有 >500 kB chunk 提示保留。 |
| `pnpm exec eslint . -f json -o <temporary-report>` | PASS：0 errors / 32 warnings。 |
| `pnpm run check:large-files --json --top 300` | PASS：87 个提示、0 blocking。 |
| `git diff --check` | PASS。 |

浏览器执行：

```text
pnpm exec playwright test apps/desktop/tests/smoke/sidebar-consistency.spec.ts apps/desktop/tests/smoke/ui-smoke.spec.ts --workers=2
```

PASS：Chromium 4 项，覆盖英中文资源侧栏、Flow/SSH 长列表和模块切换。
这些 smoke 不覆盖原生事件 channel；上述时序行为由新增 hook 测试验证。

## 剩余风险和下一轮建议

- NOT RUN：真实 SSH/native Tauri channel、Rust、MCP、Cloud Sync 集成。本轮没有修改这些边界，
  不把 mock 测试当作真实远端执行或安全验证。
- 尚未改变的竞态：清理不会取消正在读取的历史日志或未刷新的事件；同一个 hook 实例切换 workspace
  不主动清空已提交缓存；并发读取共享 loading 布尔值。是否改变需要单独明确 UI 与请求失效契约。
- 下一轮优先补环境表单的契约：可选 defaults 加载失败、手动输入保留、环境 revision 更新、
  secret 名称标记、重复 prepare/工作区切换的迟到响应。测试完整后再抽出运行表单职责。
  更改竞态语义应作为明确的行为修复，而不是混入保行为重构。
