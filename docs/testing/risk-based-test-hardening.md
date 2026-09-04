# Risk-based test hardening — 2026-08-30

审计对象：`zyqzyq/Unfour` 的 `main`，基线提交
`232bda1236eb1703088d256ca298632b330b41a3`。开始时工作区干净；通过
`git ls-remote origin refs/heads/main` 核实远端与本地一致。本记录不是
v0.9.0 的新人工验收结果，也不替代 `release-verification.md`。

环境：Windows x64、Node 24.16.0、pnpm 10.23.0、Rust 1.96.0。
CI 使用 Ubuntu / Node 20；本地通过不能替代该环境的结果。

## Coverage baseline：统计范围与行为保障分开

首先在未修改配置/测试的提交上运行 `pnpm run test:coverage` 和
`pnpm run test:rust`。随后只补齐 coverage 统计范围，再跑同一套前端测试；
最后才比较新增测试后的同范围数据。

| 测量 | 测试 | Statements | Branches | Functions | Lines |
| --- | --- | --- | --- | --- | --- |
| 原配置，仅 packages | 108 文件 / 541 通过 | 4565/9705 (47.03%) | 3502/8028 (43.62%) | 1328/3178 (41.78%) | 4302/8928 (48.18%) |
| 扩大到 packages + apps，尚未新增测试 | 108 / 541 通过 | 5023/10654 (47.14%) | 3778/8845 (42.71%) | 1434/3433 (41.77%) | 4700/9709 (48.40%) |
| 本次完成，同扩大范围 | 108 / 546 通过 | 5030/10654 (47.21%) | 3781/8845 (42.74%) | 1437/3433 (41.85%) | 4704/9709 (48.44%) |

原来的 `test.include` 会执行 `apps/*` 测试，但 `coverage.include` 只统计
`packages/*`，因此 Account、Cloud Sync、updater 的桌面代码不在原分母内。
本次保留原来的测试发现规则，增加 apps 统计和 `json-summary`，没有调整
threshold、排除难测业务文件或给无关 UI 补覆盖率。

扩大范围后的**新增测试前**模块统计如下。这些都是 TS/TSX 数据，不能作为
Rust 执行、安全或 SQLite 事务的覆盖率。

| 范围 | 行覆盖 | 分支覆盖 | 解读 |
| --- | --- | --- | --- |
| API Client | 688/1321 | 577/1101 | 不含 Rust HTTP/script runtime |
| Database | 693/1837 | 665/1857 | 低覆盖不能等同于 mutation guardrails 未测 |
| SSH Terminal | 1282/2701 | 893/2242 | terminal/任务前端，不能证明真实 SSH 行为 |
| workspace-core | 16/19 | 10/10 | 很小的共享状态文件，不代表 Workspace 生命周期 |
| workspace-environments | 166/208 | 153/198 | 编辑器及 hook；后端持久化另测 |
| Desktop Account | 132/257 (51.36%) | 77/230 (33.48%) | Rust 授权流程另有薄弱点 |
| Desktop Cloud Sync | 225/353 (63.74%) | 194/361 (53.74%) | provider/UI，核心 reconcile 在 Rust |
| Desktop updater | 35/151 (23.18%) | 5/224 (2.23%) | 下载事件/安装交互的明显测量与行为缺口 |

Rust 原基线为 **719 passed / 0 failed / 1 ignored**；最终为
**735 passed / 0 failed / 1 ignored**。忽略项是明确需要 OS credential store
的 `os_keychain_release_smoke`。仓库没有 Rust coverage 任务，本机没有
`cargo-llvm-cov`、`tarpaulin` 或 LLVM coverage 组件；Rust 行/分支覆盖率
记为 **NOT MEASURED**，不能填 0%，也不能根据测试数量推算。没有为这次审计
安装新的 coverage 工具链。

本地原始日志与原始/扩大范围 LCOV 保留于忽略目录 `logs/test-hardening/`；
最终 HTML、LCOV、JSON 位于 `coverage/`。这些生成物不提交。上述精确计数、
基线 SHA、配置变化和执行命令是仓库内的持久基线。

## 当前测试能力地图

| 链路 | 已有有效保障与证据 | 自动化边界 |
| --- | --- | --- |
| Workspace 生命周期 | `workspace-engine/src/workspace_tests`；command-bus `tests/workspace_domain` 验证作用域、级联、最后 workspace fallback、secret metadata、hook/activity/outbox 回滚 | SQLite 回滚较强；OS keychain 与 DB 联合故障、进程崩溃/磁盘故障仍薄弱 |
| Cloud push / retry | `tests/worker/push_retry.rs`、`dead_letter_recovery.rs`、`transactions.rs`：相同 operation replay、旧 head、部分 ACK、dead-letter 修复、重启 | MockTransport 是脚本化协议对端，不是持久化服务器 |
| Cloud pull / conflict / tombstone | `pull_cursor.rs`、`hierarchy_conflicts.rs`、`api_hierarchy.rs`、`workspace_conflicts.rs`：cursor 不跳过未见记录、父子冲突、keep-local/use-remote、级联与无 echo | 已有大量失败/回滚断言，本次没有复制它们 |
| Cloud snapshot / second device | `snapshots.rs`、`transactions.rs` 覆盖分批 staging、晚期失败回滚；本次 `second_device.rs` 串接实际 push payload、另一份 DB、删除与重启 | 客户端 compatibility relay 已验证；真实 hosted service 并发/幂等仍需验收 |
| OAuth / Desktop session / deep link | 原有 PKCE、callback parser、billing loopback、frontend stale refresh；本次增加 AccountService 真实 HTTP + memory keychain 的状态转换 | 浏览器 GitHub 授权、OS scheme 分发和 keychain 拒绝访问没有被 mock 测试替代 |
| Entitlement | Rust profile/code/status/expiry 与 frontend provider gate；本次测缓存 capability 隔离、撤权、失效、退出和 API 拒绝 | 30 秒缓存与真实 entitlement/session 到期交叉边界仍待加固 |
| MCP policy / confirmation | `policy_tests`、database/SSH handlers、registry smoke；本次验证参数变更与策略收紧时执行次数为零 | confirmation 是无服务端状态的请求指纹，不是一次性授权票据；真实客户端确认体验另验 |
| API pre/post scripts | runtime sandbox、循环打断、输出大小；command-bus pre failure no-I/O、pre write rollback、post response preservation；本次增加 post 部分写回滚与 history/activity/console 脱敏 | HTTP 已发送不能回滚远端业务；pre 成功提交不因 post 失败撤销 |
| SSH / SSH Tasks | session/reconnect/host-key、模板/绑定/redaction、task runner 顺序/取消；本次补失败停止、continueOnError、启动前取消 | 真实服务器超时、断流、SFTP 半成品处理不能由 fake driver 证明 |
| Database mutation guardrails | Rust safety 分类、SQLite PK/原值冲突、只读/确认、多 statement 拒绝；本次前端测试部分成功后保留失败编辑 | 多行前端提交是逐行提交，不能声称整个批次 rollback；本次明确验证该语义 |
| updater / release channel | `test:release-env` 69 项：Stable numeric SemVer、manifest downgrade/fail-closed、签名资产、RC 零发布/Store policy；Tauri metadata/进度 batching | 静态 workflow contract 不证明执行过发布，也不证明无效签名的 installer 未启动 |
| Migration | cloud-sync-storage 9 项：clean、Community、Pro 数据精确保留、重复迁移、legacy binding 与协议 bootstrap；18 个 SQL 文件 checksum guard | dirty/checksum mismatch/磁盘耗尽/中断恢复矩阵不足 |
| Secret redaction | core recursive keys、HTTP persistence、SSH tasks/history、MCP masking、diag；本次 secret canary 穿过脚本失败/日志与 second-device 数据 | 不能据此保证任意编码、任意服务器错误文本、所有 binary/大载荷都安全 |

测试层次：Vitest 负责 frontend unit/component；Rust unit/integration 负责
执行边界与真实 SQLite；Node tests 负责 release contracts；Playwright 当前只有
两个 Chromium browser smoke（模块切换、API 布局），使用 browser mocks，
**不是真实 Tauri E2E**。CI 的普通 PR job 不运行 coverage 或 Playwright，
reusable Standard build 才执行 browser smoke。

## 按风险排序的 18 个薄弱行为

排序依据为数据损失/越权影响、触发机会、现有断言能否检测错误；优先级不是
coverage 排名。`已补`表示这里描述的具体行为通过，不代表整条链路完成。
`待补`与静态审查线索不等同于已经复现的缺陷。

| 排序 | 行为与失败后果 | 审计前的缺口 | 本次 / 下一步 |
| --- | --- | --- | --- |
| 1 · P0 | logout 后旧 OAuth exchange 返回，session 被重新保存 | 只有 local sign-out/parser 测试，没有交错执行 | **已复现、修复、补测**；generation 检查与 keychain transition 锁 |
| 2 · P1 | snapshot 未到末页便被应用，第二台得到不完整 workspace | 正常 pagination 与 apply 失败已测，但极限页数耗尽未测 | **待补**：`service/snapshots.rs` 在 `MAX_REMOTE_PAGES` 循环后未显式检查剩余 page token；10,000 页边界为静态风险线索，未复现 |
| 3 · P1 | 旧 `/v1/me` / billing 的 invalid-session 响应与新登录交错 | parser/单次失效测试不能证明不会删掉更新的 credential | **已修复、补测**：`state`、`require_entitlement`、`finish_billing_request` 均按 session/generation 绑定删除；旧响应不会清掉新登录凭据 |
| 4 · P1 | entitlement/session 已到期但 30 秒 capability cache 尚未过期 | profile expiry 单测不能证明缓存不会越过有效期 | **部分已补**缓存到期/撤权；自身 expiresAt/validUntil 与缓存 TTL 交叉边界待补 |
| 5 · P1 | Workspace 级联过程中 SQLite 与 keychain 一端失败 | SQLite hook/child/activity rollback 很强，但不能代替 OS secret store 故障 | **待补** fault injection，逐项断言记录、credential reference、outbox、active fallback；不得绕开 secret-store |
| 6 · P1 | MCP confirmation 被带到另一 workspace/连接/SQL，或旧确认绕过新 policy | 原测试主要错误文本和一次 confirmed happy path，缺执行次数断言 | **已补**六种 payload 变更零执行、dry-run 零执行、policy 收紧后拒绝 |
| 7 · P1 | 第一台导出的 payload 在第二台语义不一致，删除重启后复活 | 原有快照测试多用手写独立 payload，缺两个独立 DB 的 producer→consumer 链 | **已补**实际 push payload relay、snapshot、tombstone、restart、无 echo/secret 泄露 |
| 8 · P1 | Account 切换发生在 snapshot 网络等待中，旧账号数据落入本地 | 原 generation/worker 测试不能完整代表 snapshot 安装路径 | **已补**确定性 barrier，workspace/active/binding/outbox/staging 不变 |
| 9 · P1 | post script 部分修改 environment 后抛错，污染持久状态或泄密 | pre rollback 和 post response preservation 各自已有；缺组合断言 | **已补**保留 pre commit、拒绝 post 部分写、HTTP history 恰好一条、console/error/activity canary 不泄露 |
| 10 · P1 | 批量 DB 修改前行成功、后行冲突，重试重复提交成功行或丢失未提交编辑 | hook 原来只有全部成功测试 | **已补**首行失败/中途失败两种场景，失败及未尝试编辑保留，只重试剩余行 |
| 11 · P1 | SSH Task 步骤失败仍执行后续 destructive step，或取消时序误判 | runner 原有顺序与 sleep-based 取消测试，缺失败分支 | **已补**失败停止、显式 continueOnError 保留失败事件、预取消零副作用；取消改 Notify |
| 12 · P1 | OAuth stale/malformed/replayed callback 消耗有效 attempt 或重复 token exchange | callback parser 有覆盖，service 状态转换不足 | **已补**旧 state/重复参数保留当前 attempt、denial 消耗、success replay 拒绝、失败保留旧 session |
| 13 · P1 | API 永久拒绝被当 transient 无限重试，或撤权导致误登出 | retry helper 只有成功/永久失败；服务缓存/凭据断言不足 | **已补**retry 五次/十秒虚拟预算、invalid session 清理、invalid profile 保留 credential、remote revoke 失败仍保持本地登出 |
| 14 · P1 | migration 中断或 dirty/checksum mismatch 被误当成功，重启后数据残缺 | 正常旧库/新库/idempotency 测试不能覆盖存储故障 | **待补**损坏 migration record、事务中断与恢复；继续保持历史 SQL 不变 |
| 15 · P1 | SSH/SFTP 真实断流或超时留下错误成功状态/半成品文件 | fake driver 可以证明 runner 分支，不能证明远端进程/文件状态 | **待补**受控 SSH 服务和传输故障 harness；真实 OS/服务器验收保留 |
| 16 · P1 | updater 无效签名、取消/失败后仍触发安装或跨 channel 下载 | release contract 和错误映射不等同于 installer 副作用验证；desktop branches 2.23% | **待补**先补 provider/event 状态机与受控 adapter；再加打包应用签名拒绝旅程 |
| 17 · P1 | 云端提交成功但响应丢失，重试/冲突无法跨服务重启保持幂等 | 客户端相同 operation replay、dead letter、rollback 已有，服务器是 scripted mock | **待补**持久云端 test fixture/真实双设备冲突、retry、keep-local/use-remote；不复制已有客户端用例 |
| 18 · P2 | 脱敏在异常文本/编码/大载荷边界漏掉嵌套 credential | 单个 helper 的敏感键覆盖不能代表每条 persistence/LLM 路径 | **部分已补**脚本错误与 push/snapshot canary；下一轮补编码/截断/大小写的端到端负向 corpus |

## 本次新增与脆弱测试修复

新增 **21 项测试**：Rust 16、Vitest 5；没有新 Playwright，也没有复制现有
workspace cascade、cloud transaction 或 release schema 测试。

- AccountService：8 项，涵盖以上 OAuth、cache、credential 与 logout 流程。
- Account retry：1 项，失败预算耗尽；原成功重试测试改为虚拟时间并断言延迟。
- Cloud Sync：2 项，真实客户端 payload 的 second-device relay，以及 account
  switch 中断 snapshot；现有 85 项 worker 测试保留。
- MCP：2 项，六维 payload binding、dry-run 与实时 policy；记录执行调用，
  不能只凭返回错误推断未执行。
- API scripts：1 项，post 部分写回滚、保留 pre commit/history、脱敏。
- SSH Tasks：2 项，失败/continueOnError 和预取消；原取消测试改事件同步。
- 前端 entitlement：3 项，精确到期、等价 UTC+8 时间、非法日期均不得授权。
- 前端 DB mutations：2 项，首行/中途失败及之后只重试剩余编辑。

修复 **8 个现有脆弱测试**：Cloud Sync 的 in-flight pause、global resume、
singleflight coalescing 三项移除 10/20/80ms 调度假设；SSH 取消移除 20ms
调度和 2s 假成功；Account retry 移除真实 1+2s 等待；PostgreSQL 两项与
MySQL 一项使用一直持有的 `127.0.0.1:0` 拒绝端点，移除 TEST-NET 地址和
固定 5432/9 端口假设。PG secret 测试现在真正提供 secret canary。

新 HTTP fixtures 与 DB 拒绝端点有 Drop 清理；关键 gate 有有界超时，失败不靠
无限 sleep。剩余旧 HTTP helper 中的阻塞 `accept()`、server tests 的真实延迟、
临时文件清理、OS-native/keychain 路径仍是下一轮 flake 审查对象。
UUID/PKCE 随机性用于唯一标识而非断言幸运输出；没有修改全局随机数或系统时钟。

## 验证与诚实边界

| 命令 | 结果 |
| --- | --- |
| `pnpm run test:coverage`（原配置、扩大范围、最终，各一次） | PASS；541 → 546，精确覆盖数据见上 |
| `pnpm run test -- packages/database/src/hooks/useTableRowMutations.test.tsx apps/desktop/src/features/account/accountEntitlement.test.ts` | PASS，16 项 |
| `pnpm run test:rust`（原基线与最终） | PASS，719 → 735；1 OS keychain ignored |
| `cargo test -p unfour-account service_tests` | 初始复现 logout/exchange 缺陷；修复后 PASS 8 项 |
| `cargo test -p unfour-command-bus script_rollback` | PASS 1 项 |
| `cargo test -p unfour-cloud-sync --test worker` | PASS 87 项 |
| `cargo test -p unfour-mcp tools::database::tests::execute` | PASS 5 项；纠正初次过滤器未匹配测试的问题，全量 Rust 也覆盖了这些测试 |
| `cargo test -p unfour-database-engine` | PASS 38 项；受控失败端点组无外部 DB |
| `cargo test -p unfour-ssh-engine --features ssh-native` | PASS 88 项；不声称连接过真实 SSH 服务器 |
| `pnpm run test:e2e`，`CI=true` | PASS 2 Chromium smoke |
| `pnpm run test:release-env` | PASS 69 项 |
| `pnpm run build` | PASS；仍有 >500KB chunk 提示 |
| `pnpm run lint` | PASS；0 errors / 89 warnings |
| `pnpm run check:rust`、`pnpm run check:rust:ssh` | PASS |
| `cargo fmt --all -- --check`、`git diff --check` | PASS |
| `pnpm run check:version`、`check:migrations`、`check:tokens`、`check:large-files` | PASS；18 migration files，107 tokens，0 blocking size violation |
| `pnpm run check:secrets` | PASS；扫描本次待发布文件，未发现 secret |

另外将 AccountService 8 项、Cloud Sync accounts 4 项、scheduling 3 项各重复
10 次：150 次测试执行全部通过。此结果用于验证新事件同步 fixture 的稳定性，
不代表已经证明不存在任何 flaky。

Windows sandbox 起初以 `EPERM spawn` 拒绝了 coverage、Node release tests、
secret scanner 的子进程；已通过允许的沙箱外执行重跑通过。此类基础设施失败
没有被隐藏或归类为业务测试失败。编译/fixture 编写期间的错误也已修正，最终
通过的是实际重新执行的测试。

`pnpm run test` 未另跑全量无 coverage 模式，因为最终 coverage 已执行同样
108 文件/546 测试；上表另有聚焦无 coverage 测试。未执行 Rust line coverage，
理由见 baseline；未安装依赖、启动真实付费服务、发布版本或推送远端。

## Manual verification 与下一阶段

以下本次均为 **NOT VERIFIED**，不能沿用历史 v0.9.0 PASS 当作本次执行：

- GitHub 浏览器 OAuth → closed/running desktop deep link → OS keychain 保存，
  含用户取消/拒绝 keychain 权限；NSIS/MSIX scheme 共存。
- 两台真实设备连接同一 staging cloud：离线编辑、push/pull、冲突两种选择、
  tombstone、snapshot、响应丢失后 retry、两端重启。
- Creem Production 的真实支付/webhook/entitlement，真实 MCP prod policy 的
  read-only、确认文字/payload 绑定和重试 UX。
- 打包 updater 的错误签名拒绝且 installer 不启动、Store 不调用内部更新器；
  Windows/macOS/Linux 平台信任、服务与路径差异。
- 真实 SSH/SFTP 的服务器断连、timeout/cancel 后远端进程/文件状态，以及
  PostgreSQL/MySQL 的事务、权限/TLS、并发 mutation 冲突。

这些并非永远无法自动化：先补受控 adapter/服务集成测试，只有 OS/browser/
打包应用分发边界才值得引入新的 E2E。不要用更多 browser mock smoke 替代它们。

下一轮优先：排名 2–5 的 snapshot 完整性、旧 session response、缓存有效期、
跨存储故障；随后 migration fault matrix、SSH/SFTP 故障、updater 副作用。
CI 建议先归档同范围 coverage JSON/LCOV，分开运行确定性 unit/integration
和需要受控服务的 job，并在 Windows/Linux 至少覆盖一次关键 Rust 状态机。
不要先设置全仓覆盖率门槛；采用明确危险行为的验收清单。

低价值测试没有达到需要批量删除的程度。可将 `packages/ui/src/badge.test.tsx`
四个 markup/class 检查合并成一个渲染与 variants contract；Account entitlement
中“Free→Pro / Pro→Free”只是重复 pure-function 调用，可与 status 表驱动矩阵
合并。保留 Button 的 disabled/click/asChild、hook failures、真实 SQLite rollback、
release 静态 contract；它们不是因为实现短就无价值。本次不为减少数量删除测试。

## 修改文件与人工审阅

| 文件 | 目的 |
| --- | --- |
| `vitest.config.ts` | apps coverage、机器可读 JSON summary |
| `apps/desktop/src/features/account/accountEntitlement.test.ts` | expiry/时区/非法日期边界 |
| `packages/database/src/hooks/useTableRowMutations.test.tsx` | 部分成功、失败保留、重试范围 |
| `crates/unfour-account/Cargo.toml` | 显式启用已有 Tokio 的 sync；dev-only test-util 虚拟时间 |
| `crates/unfour-account/src/lib.rs` | 唯一生产逻辑修复：exchange 与 sign-out 串行化 keychain transition，拒绝旧 generation |
| `crates/unfour-account/src/client.rs` | 仅测试修改：虚拟 retry 时间、耗尽预算 |
| `crates/unfour-account/src/service_tests.rs` | 8 个 AccountService 行为回归 |
| `crates/unfour-account/src/service_tests/support.rs` | 受控 loopback HTTP、memory secret store、清理 |
| `crates/unfour-cloud-sync/tests/worker.rs` | 注册 second-device suite |
| `crates/unfour-cloud-sync/tests/worker/second_device.rs` | 两份 DB 的 snapshot/tombstone/restart；切账号丢弃 snapshot |
| `crates/unfour-cloud-sync/tests/worker/accounts.rs` | pause/resume 测试改 barrier |
| `crates/unfour-cloud-sync/tests/worker/scheduling.rs` | singleflight 确定性交错及一次 dirty follow-up |
| `crates/unfour-cloud-sync/tests/worker/support/transport.rs` | 删除 sleep 注入，支持同一 cloud ID 的双端 fixture |
| `crates/unfour-command-bus/src/lib_tests.rs` | 只注册 post-script 回滚模块 |
| `crates/unfour-command-bus/src/lib_tests/script_rollback.rs` | 失败、持久化、脱敏组合断言 |
| `crates/unfour-mcp/src/tools/database_tests/execute.rs` | confirmation/policy 的零副作用及 payload binding |
| `crates/database-engine/src/database_tests/support.rs` | 持有随机 loopback 端口、拒绝握手、Drop 清理 |
| `crates/database-engine/src/database_tests/postgres.rs` | 移除外网/固定端口假设，真实 secret canary |
| `crates/database-engine/src/database_tests/mysql.rs` | 移除固定端口假设 |
| `crates/ssh-engine/src/task/runner.rs` | 仅测试修改：Notify、失败/继续/预取消 |
| `docs/testing/risk-based-test-hardening.md` | 本次可复现基线、能力地图、风险与验证记录 |

业务逻辑：**有**，仅 AccountService 竞态修复。新依赖：**无**（仅已有 Tokio
features）。包依赖方向、公开契约、Tauri/MCP adapter 与 command-bus 调用链：
**未改变**。跨包修改用于在各自拥有的测试边界验证风险，没有将业务或测试接口
搬到 app-shell/ui。`lib_tests.rs` 仍约 795 行；新增逻辑放在按责任划分的子模块，
没有为行数重构无关旧测试。

人工优先审阅：AccountService transition 锁/generation 与回归测试；second-device
fixture 的服务器模拟边界；MCP RecordingBus 的 payload/policy 零调用断言；
DB 逐行提交而非批次事务的失败语义；本报告中排名 2–5 的剩余风险。
