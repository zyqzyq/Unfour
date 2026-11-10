# AccessClient A1：本机只读证据与接入门槛

关联 [总任务 #3](https://github.com/zyqzyq/Unfour/issues/3)、
[A1 #4](https://github.com/zyqzyq/Unfour/issues/4)。检查日期：2026-11-10（Asia/Shanghai）。
仓库基线：`0dc4460d1619c9d85c360006cf5d4103a93dc759`。

**状态：A1 部分完成；真实 AccessClient → Xshell 参数契约尚未确认。**
后续状态：PR #8 已合并；用户授权先实施不依赖未知格式的 A2/A3 基础能力，
见 [基础验证报告](bastion-temporary-session-foundation-2026-11-10.md) 和
[现场观察步骤](bastion-field-observation.md)。以下“未实现／未运行”表格保留为 A1 当轮历史，
不覆盖后续基础测试结果；真实入口和现场验收门槛不变。
本次没有连接客户环境、启动 AccessClient/Xshell 连接、修改注册表或客户端配置、替换程序。
未确认的参数不能用官方 JumpServer 客户端或 ShellRS 的格式代替。
A2/A3 可以根据[内部边界草案](../architecture/bastion-launch-contract.md)设计；
生产外部入口与认证适配仍受下述门槛约束。A2 不阻塞 A3；所有关联 Issue 保持开放。

## 证据等级

- **用户确认**：用户描述的既有现场行为，未由本轮复测。
- **本机静态确认**：注册表、安装文件或 PE 版本/静态字符串；不证明执行分支和授权行为。
- **上游源码确认**：仅适用于指定仓库、提交、文件，不代表齐治客户端。
- **待验证**：缺少动态观察、版本信息或一次性授权环境。
- **不支持**：产品明确未实现，或未来现场明确拒绝；不能用“未知”推断远端不支持。

## 本机发现

| 项目 | 只读结果 | 能证明 / 不能证明 |
| --- | --- | --- |
| 既有链路 | JumpServer 网页 → QizhiTech AccessClient 4.6.15 → Xshell；网页无自定义客户端项 | 用户确认；本轮未复测 |
| 安装记录 | AccessClient 4.6.15，目录 `<ProgramFiles(x86)>\QizhiTech\AccessClient` | 安装元数据确认；不包含服务器版本 |
| LoaderShell | `LoaderShell.exe` 文件/产品版本 `4.6.15.0`；SHA-256 `ac6bc8ab1435f72d0a752be618ceed5cd94c7a1af9fd4304579474ff7355473c` | 固定本次静态样本；不是签名或运行行为证明 |
| 浏览器协议 | HKLM 32/64 位视图 `Software\Classes\accessclient\shell\open\command` 和 `accessmac` 对应项均为 `"<install-dir>\LoaderShell.exe" "%1"` | 浏览器将一个带引号的参数交给 LoaderShell；未读取任何真实启动 URL，也未确认其载荷结构 |
| Xshell 路径 | HKLM 32/64 位视图 `Software\Microsoft\Windows\CurrentVersion\App Paths\Xshell.exe` 默认值指向现有 `<ProgramFiles(x86)>\NetSarang\Xshell 8\Xshell.exe`；该项只有默认值 | Windows 应用路径注册已确认；AccessClient 查询优先级、缓存、配置 UI、任意程序路径是否获准均未知 |
| Xshell 版本 | 卸载记录 `8.0.0119` | 客户端安装版本；不是调用协议版本 |
| LoaderShell 路径 | 相同 App Paths 根下 `LoaderShell.exe` 指向安装目录中的同名文件 | 应用路径注册；不改变浏览器关联 |
| WinSCP 路径 | 在检查的 HKCU/HKLM 32/64 位 App Paths 中未找到 `WinSCP.exe`；相关卸载记录也未找到 | 不能排除便携版、自定义目录或尚未配置；末端路径位置仍未确认 |
| 安装目录配置 | 目录中未见可确认的 Xshell/WinSCP 路径配置文件；存在 `RDCShell.exe.config`，未为 SSH 接入读取其内容 | 不能排除启动时生成、其它位置或注册表保存的配置 |
| 配置定位范围 | 检查安装目录；HKCU/HKLM Software 及 WOW6432Node 顶层相关名称；AppData/ProgramData/用户与公共 Documents 的相关目录名称；上述 App Paths/协议项 | 未找到明确的 AccessClient 路径配置；未递归搜索个人文档、浏览器数据、连接日志或 Xshell 会话 |
| 辅助程序 | 安装目录有 `sessproxy-client.exe`、`gocurl.exe`、`Accessmac.exe`、`putty.exe`、`RDCShell.exe`、`AccessClientPlugin.ocx`、`GuiPlayer.jar`、`GuiViewer.jar` | 仅证明文件存在；不能据此断言本次 SSH 使用哪个辅助进程 |
| 进程 | 检查时未列出 LoaderShell、sessproxy-client、Xshell、WinSCP 进程 | 无动态进程/端口/生命周期观察 |

LoaderShell 的 PE 静态字符串还提供以下**线索**（文件偏移，不是源码行号）：

| 字符串 / 偏移 | 限定解释 |
| --- | --- |
| `Software\Microsoft\Windows\CurrentVersion\App Paths\`，`register path`，`find path`，可执行文件选择过滤器，约 `0x18e500`–`0x18e900` | 包含路径发现/登记相关代码线索；不证明任意路径配置成功或调用顺序 |
| `XshellLoader`、`XshellProxyLoader`、`WinSCPLoader`、`WinSCPProxyLoader` 的 RTTI 名称 | 普通与代理加载器存在；本次用哪个分支未知 |
| `sessproxy-client.exe` (`0x18e848`)，`listen on:`、`PeekNamedPipe`、`CreatePipe`、`in proxy mode` | 代理与进程通信线索；监听地址、动态端口、父子存活依赖未确认 |
| `-url` (`0x191520`)，`/sessionname=`（可打印片段约 `0x1913d2`） | 二进制含这些字面量；不是完整 argv 模板，不能据此确定认证字段、URI 编码或参数顺序 |

没有输出完整配置、日志、注册表导出或命令行。公开路径用安装根占位符；
静态检查结果不包含客户地址、账户、票据用户名或原始 URL。

## 调用方式与启动器决策

当前能确认的调用边界仅为：

```text
浏览器协议处理命令："<install-dir>\LoaderShell.exe" "%1"
LoaderShell 输入：一个协议参数；载荷不透明，不解码、不公开。
LoaderShell → Xshell：未知（argv/URI/会话文件/其它 IPC 尚未确认）。
```

**尚不能直接指定 Unfour，也没有证据证明必须增加兼容启动器。**
优先保留 AccessClient 的浏览器入口、认证/授权与可能的代理职责。
获得配置入口、实际末端参数及进程存活约束后再选择：

1. 若可配置完整可执行路径，参数可由 Unfour 安全识别且无需维持子进程：直接指定 Unfour。
2. 若已证实需不同参数转换、固定程序识别或子进程存活代理：评审最小兼容启动器及其打包、退出和资源归属。
3. 若仍未知：保持入口未启用。不得抢占 `accessclient:` / `accessmac:`，不得替换或改名覆盖 Xshell。

App Paths 的存在本身不是修改该注册表项的操作指南。
本轮没有生成注册表变更文件、AccessClient 配置补丁或替换启动器。

## 最小待补材料

仅需在授权的离线测试环境使用一次性测试会话补齐以下材料，不提供原始敏感捕获：

1. AccessClient 的末端路径设置入口及字段名；如没有 UI，提供厂商配置说明。
   只报告路径形状、是否接受非 Xshell 文件名/带空格路径，以及查找优先级。
2. **末端**启动结构：接收程序、argv 数量/边界、固定开关、值分隔符、编码层数、
   是否 URI/文件/IPC；用全新合成值重建等价语法，保留引号、空格、百分号转义和非 ASCII 情况。
   标明来自进程的解析后 argv 还是 Windows 原始命令行；二者不可混用。
3. 认证类别（临时密码、票据用户名、交互认证等），端点类别（本地代理或远端入口），
   是否动态端口；只给类别与合成端口，不给真实值。不索取密钥、密码或票据。
4. 同一次性会话中的进程关系与时序：谁创建代理、何时就绪、末端进程退出是否关闭代理、
   代理退出是否断线。使用进程别名而非原始命令行/PID；不要终止正在使用的客户会话。
5. 可取得的服务器版本与独立 WinSCP 入口说明；WinSCP 另需文件传输测试授权。

第 1–4 项决定 A1 最小交付是否足以启用 A2/A3 的实际入口适配。内部基础能力已获准先行；服务器版本缺失需记录，
但原生 exec/SFTP/多通道等后续能力未知不单独阻塞 A1；这些留在 A4 分项验收。

## 合成结构与回归材料

[`launch-cases.json`](../../scripts/bastion/fixtures/launch-cases.json) 明确区分：
本机浏览器包络、其它上游客户端参考、未来边界/拒绝场景。
`accessClientHandoffVerified` 固定为 `false`，没有把候选参数当成现场捕获。

下例仅用于参数边界说明；来自上游模板的合成展开，不代表本机 AccessClient：

```json
["-newtab", "合成测试 甲", "-url", "ssh://SYNTHETIC_USER:SYNTHETIC_PASSWORD@asset.example.invalid:2222"]
```

若此数组对应 Windows 命令行，参数中的空格需要调用方正确引用，例如：

```text
"C:\Synthetic Folder\Unfour.exe" -newtab "合成测试 甲" -url "ssh://SYNTHETIC_USER:SYNTHETIC_PASSWORD@asset.example.invalid:2222"
```

该例未运行 Windows 参数解析测试，不能作为实际 CreateProcess/CRT 行为证据。
fixtures 另含 URI 外层引号、大小写、百分号编码、歧义票据分隔符、畸形编码、
未知开关、脚本/命令注入形状、会话文件路径、重复/缺失参数与控制字符。

[`launch-shape.mjs`](../../scripts/bastion/launch-shape.mjs) 是离线证据脱敏工具，
**不是连接解析器，不接触注册表、进程、文件、网络或 Unfour 数据库**。
从 stdin 读取一个不含可执行文件的 JSON argv 数组，只输出固定枚举、布尔值与位置。
不输出主机、端口、用户名、密码、标题、路径、未知开关名或 URL 载荷；不解码百分号或协议载荷。
输入错误、超限一律输出固定错误并退出 2；成功退出 0。

```powershell
# 只演示合成数据；不要把真实值粘贴进 shell、聊天或命令历史。
'["-url","ssh://SYNTHETIC_USER@asset.example.invalid:2222"]' | node scripts/bastion/launch-shape.mjs
node --test scripts/bastion/launch-shape.test.mjs
```

工具保留 argv 边界与有限的词法特征，**有意不保留完整语法或做认证字段解析**。
输出不能独自证明调用契约；仍需第 2 项的现场合成重建。
只有已批准的本地采集路径才能向 stdin 提供真实数据，不保存原始输入，
不通过命令行传递；Node 字符串不会保证内存清零，因此它不进入产品认证路径。
输出中 `uri-shaped` 只说明词法形状，畸形 URI 也可能得到该标签，不表示可连接或被接受。

## 初始能力矩阵

| 能力 | 证据状态 | 下一步独立证据 |
| --- | --- | --- |
| 既有网页 → AccessClient → Xshell | 用户确认 | A4 固定版本复测 |
| 浏览器 → LoaderShell 协议命令 | 本机静态确认 | 实际 argv 合成结构 |
| AccessClient → Unfour 冷/热启动、UI 未就绪与重复请求 | 待验证，未实现 | A2 组件测试 + A4 包测试 |
| 工作区临时终端及内存认证 | 待验证，未实现 | A3 生命周期/持久化边界测试 + A4 |
| 原生 exec | 未知 | 独立授权 exec 通道；不以终端命令成功代替 |
| 可靠远端退出码 | 未知 | exec 成功/非零退出；收到明确 exit-status |
| SFTP 入口、认证与上传完整性 | 未知 | 独立 WinSCP 入口/授权、一次无害文件及摘要 |
| 第二次登录 | 未知 | 单独一次性授权，区分旧凭证复用与新网页授权 |
| 并发通道 | 未知 | 独立授权的额外通道测试 |
| 顺序复用通道 | 未知 | 同一已认证连接先关闭后再次打开通道 |
| 凭证有效期/授权失效 | 未知 | 有效期内、到期前后观察；不能把网络断线等同过期 |
| 代理退出/末端进程退出 | 未知 | 别名进程时间线与连接结果 |
| 传输断线 | 未知 | 受控断线；远端是否完成未知时明确记为不确定 |
| 临时目标 SSH Task / Flow | 本阶段不支持 | 后续独立任务，不自动执行 |

所有“未知”均为未验证，不是已证明远端不支持。终端成功不会升级其它行。

## 上游参考与逐文件许可核对

只用于设计比较与测试场景；没有拷贝上游实现、测试数据或二进制。
检查了列出的文件及适用 LICENSE/manifest，未发现这些文件的更窄许可声明。

JumpServer client 固定提交：`a238802608a7953dd5ee0357344cf78269dd944f`。
[根 LICENSE](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/LICENSE) 为 MIT。

| 文件（该提交） | 许可依据 | 参考内容及与本机的区别 |
| --- | --- | --- |
| [windows.xshell/connect.json](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/plugins/windows/windows.xshell/connect.json) | 根 MIT；无文件内覆盖 | `user_path` + `-newtab {name} -url {protocol}://{username}:{value}@{host}:{port}`；不是齐治证据 |
| [windows.xshell/manifest.json](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/plugins/windows/windows.xshell/manifest.json) | 根 MIT；无文件内覆盖 | 声明 ssh/telnet；本阶段不因此支持 telnet |
| [windows.winscp/connect.json](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/plugins/windows/windows.winscp/connect.json) | 根 MIT；无文件内覆盖 | 独立 user_path 与 URI 模板；不证明本机 SFTP 授权 |
| [windows.winscp/manifest.json](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/plugins/windows/windows.winscp/manifest.json) | 根 MIT；无文件内覆盖 | 独立 filetransfer/sftp 类别 |
| [client_launcher.rs](https://github.com/jumpserver/client/blob/a238802608a7953dd5ee0357344cf78269dd944f/src-tauri/src/commands/client_launcher.rs) | 根 MIT；无文件内覆盖 | GUI 启动辅助客户端并传 URL；其 URL/子进程 stderr 日志方式不适用本任务 |

ShellRS 固定提交：`ad81f42c05f048e3a3539ba1a32a8ccbe2469abd`。

| 文件（该提交） | 许可依据 | 参考内容及不采用的行为 |
| --- | --- | --- |
| [src/cli/link.rs](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/src/cli/link.rs) | 根 [Cargo.toml](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/Cargo.toml) 指定 GPL-3.0-only，根 [LICENSE](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/LICENSE)；无文件内覆盖 | 参考大小写/引号/重复输入场景；不复制实现。其仅隐藏密码的 Debug、忽略未知开关等做法不满足本任务票据用户名和严格入口要求 |
| [crates/shellrs-ssh/src/session.rs](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/crates/shellrs-ssh/src/session.rs) | 独立 crate [Cargo.toml](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/crates/shellrs-ssh/Cargo.toml) 与 [LICENSE](https://github.com/since2006/shell-rs/blob/ad81f42c05f048e3a3539ba1a32a8ccbe2469abd/crates/shellrs-ssh/LICENSE) 为 MIT；无文件内覆盖 | 区分 PTY shell、exec、SFTP；不移植额外 probe；返回输出不等于可靠 exit-status |

## 本轮验证

| 检查 | 结果 | 范围 |
| --- | --- | --- |
| `node --test scripts/bastion/launch-shape.test.mjs` | PASS：14/14 | 合成结构、秘密不回显、畸形输入、限额、stdin CLI/退出码；不是产品连接解析器测试 |
| `node scripts/audit-public-secrets.mjs` | PASS | 仓库公开秘密规则扫描；另人工检查新增证据仅含占位/合成数据 |
| 新增文档本地链接解析 | PASS：17 个 | 文件存在性；上游文件另通过固定提交读取核对 |
| `node scripts/check-large-files.mjs` | PASS：0 blocking | 报告已有大文件；新增文件没有触发阻塞 |
| `git diff --check` | PASS | 提交前检查 |
| A2/A3 产品解析器、启动转发、临时连接、OAuth/metadata 行为回归 | NOT RUN | 本轮不修改产品入口或 Rust/React 行为；所需 A1 参数仍缺失 |
| Windows 安装包与 A4 客户现场能力 | NOT RUN | 当前不能连接客户环境，且 A2/A3 尚未实现 |

首轮脱敏工具测试为 13/14：测试哨兵中的下划线不是合法 URI scheme 字符，
导致测试错误期待 `other`。改用合法合成 scheme 后复测 14/14；没有放宽 URI 识别来迁就测试。
仓库 ESLint 配置排除 `.mjs`，本轮没有把被忽略的 lint 命令记作通过。
后续离线材料见 [A4 携带清单](bastion-offline-handoff.md)。
