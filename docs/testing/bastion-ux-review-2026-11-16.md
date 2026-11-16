# 堡垒机终端交互修复与复审

日期：2026-11-16。范围：当前分支已经打通的 AccessClient Xshell / SecureCRT
启动链路的节点显示、主机指纹交互、剪贴板、光标，以及相邻终端竞态。
本轮使用隔离测试数据，未连接客户内网或重新交付 Windows 离线包。

## 修复结果

### 节点与连接地址

原入口解析器主动丢弃外部 label，临时摘要只有句柄，所有标签显示固定名称。
现在两种入口均保留经过清理的显示名称；标签、上下文和 SSH / Database / Redis /
API 转发选择器统一显示 `节点名称 · host:port`，无名称时使用连接地址。

`endpoint` 是实际 SSH transport 地址。若 AccessClient 提供本地代理地址，界面不将
其推断为下游资产 IP；下游节点名称只使用上游明确提供的 label。
显示名称不得含完整用户名、密码或 URI，清理控制/双向字符后再次检查，限制为
160 字符加省略号。只增加内存摘要 metadata，不保存普通连接、历史或云同步记录。

### 指纹交互

去掉每次临时启动的“仅本次信任”操作，采用自动首次使用信任（TOFU）。已有持久
指纹仍必须匹配；DNS 大小写的不同拼写若存在多个记录，全部必须匹配。新指纹仅
在内存按工作区、主机名、端口记录，同工作区后续启动必须一致，变化返回明确错误。

记录在当前打开工作区的生命周期内有效：关闭单标签保留，关闭工作区/切换离开或
退出应用释放。最多 1024 个，不用淘汰旧 pin 的方式重新接受未知密钥。此策略不提供
跨工作区关闭、应用重启后的首次身份保证，也不把堡垒机授权当成绕过指纹检查的理由。
旧 trust 命令保留兼容入口但不能覆盖拒绝结果。

查库后的取消/归属检查、并发首次连接的原子比较、rekey 关闭策略继续有效。
去掉指纹等待后传输建立上限为 30 秒；认证和 PTY/shell 仍分别为 30 / 15 秒。

### 剪贴板与光标

- 终端菜单关闭不再通过下一帧抢回焦点，避免与首次权限弹窗竞争。
- 首次读取的 `NotAllowedError` 只有在权限明确为 `granted` 后才恢复焦点、等待
  文档重新获得焦点（最长 1 秒）并重试一次；拒绝授权和持续失败仍正常报错。
- 异步粘贴绑定原工作区、会话及可见/可写状态。切换、隐藏、断开、禁用输入或卸载
  后丢弃迟到结果，防止误贴入新终端；等待期间阻止重复读取。Ctrl+V 仍由 xterm
  原生粘贴路径处理，不重复发送。
- 普通与堡垒机终端本来就共用 xterm，未发现堡垒机特有的闪烁频率设置。DOM 光标
  从每 500ms 明暗切换放慢为 700ms（完整周期 1.4 秒），支持系统减少动画，保留
  远端对光标形状和显示/隐藏的控制。
- 复审补齐连接树复制主机/SSH 命令、SFTP 复制路径的失败反馈；“复制错误”在 API
  不可用或权限拒绝时不再假报“已复制”。复用共享反馈和既有翻译，无新依赖。

### 同类竞态复审

修复关闭标签与迟到列表/输出竞争造成的标签复活和误报、单个过期终端阻断其他
终端输出、成功轮询后错误不清除、重复认领/启动误报，以及旧工作区关闭回调污染
新工作区、旧标签关闭把新选中标签切回保存连接视图的问题。

独立复审确认标签清洗后重新检查敏感值、异步粘贴目标校验、持久与内存指纹匹配。
验包脚本同步更新为无信任按钮、显示节点和地址；初始 resize 的采样从进程启动前
开始，关闭标签必须等待消失后再测试下一次同名启动。

## 验证

| 结果 | 检查与证据 |
| --- | --- |
| PASS | 临时 SSH 引擎回归 16 项（15 项套件及补充的查库期间取消用例）：TOFU、持久/内存 mismatch、DNS 大小写冲突、并发首次连接、缓存上限、取消、过期、零持久化、显示 metadata。 |
| PASS | `cargo test -p unfour-command-bus --features ssh-native temporary_ssh_commands::tests --offline --lib`：6 项本机真实 SSH 回环测试；自动连接、单次认证/PTY、零数据库写入、转发/目标认证、拒绝、取消、超时、rekey 关闭。最终指纹实现下已复跑。 |
| PASS | `cargo test -p unfour-app launch:: --lib`：11 项入口/参数/摘要测试。 |
| PASS | 13 个相关前端文件 86 项 Vitest：临时标签/竞态、终端输入输出和剪贴板、路由选择与失效、Redis/Database/SSH 表单。 |
| PASS | 同类复制入口 3 个文件 24 项 Vitest；共享 i18n provider 7 项。合计相关前端单测 117 项。 |
| PASS | `node node_modules/@playwright/test/cli.js test apps/desktop/tests/smoke/ssh-terminal-interactions.spec.ts --workers=1`：1 项实际 Chromium xterm DOM 测试；合成权限恢复、焦点、1.4 秒光标周期、减少动画。 |
| PASS | TypeScript 检查、Monaco 准备及 Vite 生产构建；`cargo check -p unfour --features ssh-native --offline`。 |
| PASS | 三个改动 Rust crate 的格式检查、改动 TS/TSX 的 ESLint、`git diff --check`、验包脚本语法检查。ESLint 有 8 个既有函数规模/复杂度警告，无错误；Vite 保留既有大 chunk 警告。 |
| PASS | `node scripts/check-large-files.mjs`：无 blocking，既有文件规模告警未作为无关重构扩展。 |
| NOT VERIFIED | 真实 WebView2 首次权限弹窗、客户 AccessClient 内网链路和主观光标体验；Chromium 用合成权限结果，不能代替现场复验。 |
| NOT RUN | 本轮未重新构建完整 Windows 离线包，更新后的 `verify-windows-package.mjs` 未针对新包执行。 |

执行期间曾遇到沙箱内 pnpm 的 `unable to open database file`，改用已安装 Node CLI
完成前端检查；本机回环测试在沙箱内握手超时，允许回环访问后全部通过。首轮 Redis
测试/typecheck 的新路由 metadata 类型失配已修复；竞态测试首次按钮文案匹配错误已修复。
一次 Vite CLI 错误使用 `--root` 导致未执行构建，改在 desktop 目录运行后通过。
以上失败均已复验，未跳过失败断言或降低校验。

## 提交前最终复审

2026-11-16 独立复审 Rust 信任/启动/敏感字段边界、前端生命周期/剪贴板/路由
consumer，未发现阻塞问题。复审纠正了 scrt 历史包记录与本轮未打包 UX 行为混用的
文档表述；原包现场登录反馈和本轮自动化结果分别记录。新增提交前公开秘密扫描
PASS（2067 个可发布文件），格式和 diff 检查通过；无产品代码变更需要重复已过套件。

按用户确认的阶段范围，#4–#6 的启动契约与临时终端实现可收口。#7 仍为部分现场
验收，#3 仍跟踪临时目标 Task/Flow 和离线部署；关闭开发 issue 不代表这些后续能力
验收通过。具体未完成项保留在 GitHub #3/#7，证据链接随提交同步。

契约见 [堡垒机启动架构](../architecture/bastion-launch-contract.md)。用户原有未跟踪的
`architecture-roadmap-review-2026-11-13.md` 保持不动。
