# AccessClient 不支持参数的启动诊断

本页记录先前的诊断包。收到现场脱敏报告后，后续包已增加固定 scrt 参数兼容，
当前支持范围见 [scrt 离线验收说明](bastion-scrt-launch-2026-11-16.md)。以下诊断行为继续
用于不支持的调用格式；下文“尚未支持”描述诊断包构建时的状态。

本次只增加失败诊断，尚未支持 SecureCRT (`scrt`) 的连接参数。当前现场已确认的
Xshell `-url <SSH URL> -newtab <label>` 入口保持不变。未改 AccessClient 配置、
注册表或客户环境。用户现有工作区不用于本机验收。

## 离线操作

普通 ZIP 包含应用、MCP sidecar、本机回显服务和说明，不重复携带运行库；旧包已能启动
的机器可使用它。文件名带 `-full.zip` 的完整包另外附有原来的 WebView2 和 VC x64
离线安装程序。两种包的 `unfour.exe` 相同。

1. 退出旧测试程序并备份旧测试目录。将新的 Windows x64 测试包解压覆盖原测试目录，
   保持 AccessClient 的 scrt 已选中的 `unfour.exe` 路径。如果解压到新目录，需要重新
   选择新路径，不能继续启动旧文件。已有离线运行库可以继续使用。
2. 网页选择 scrt，触发一次已授权的测试连接。
3. 参数尚不支持时，Unfour 在启动工作区前显示“启动诊断 / Launch diagnostic”。
   不尝试认证，不加载会话文件，不运行脚本或命令，不转交已有实例。
4. 弹窗展示前 12 个参数的脱敏结构。完整报告在资源管理器地址栏输入
   `%TEMP%\Unfour-launch-diagnostics` 后可见，文件名为 `launch-<随机值>.txt`。
   多次启动各保存一份报告，按修改时间取本次文件。只返回该文件或诊断弹窗截图。
   不返回任务管理器原始命令行、URL、会话文件、客户账户或授权材料。
5. 点击“确定”关闭诊断，该次进程退出码仍为 2。已运行的 Unfour 会话不受影响。
   若报告保存失败，弹窗明确说明，可提供弹窗截图。若完全没有弹窗，尚不能认定
   AccessClient 已执行了本次程序，应继续检查末端路径选择。

## 数据边界

- 所有输出由固定字符串、参数索引、计数和结构标记生成，不复制输入片段。
- 仅识别白名单开关。用户名、票据、密码、域名、端口、标签、文件路径及未知参数
  均省略。带 `=` 或 `:` 的已知开关只保留开关和分隔符，后面的值整体省略。
- 已知带值开关的下一个参数按值处理，即使其内容恰好等于 `/SSH2` 等开关。
- URI 只报告固定 scheme 类别及分隔符是否存在，不解码，不读取其指向的文件或服务。
- 记录 Windows 已拆分的 argv 边界。原始命令行的引号已被拆分，无法从本报告还原。
  `literal-double-quote` 只代表拆分后仍存在的字面引号。
- 最多记录 64 个参数，过长参数、非法 Unicode 和未知开关均使用固定标记。
- 报告只保存在系统临时目录，不进入工作区数据库、活动、可恢复历史或 Cloud Sync。
  不自动上传，不自动复制剪贴板。报告不含真实用户目录路径，弹窗使用 `%TEMP%` 表示。
- 启动失败发生于 Tauri/WebView2 和前端 i18n 初始化之前，使用固定中英双语原生提示。
  新增 Windows 专用 `windows-sys` 直接依赖以调用系统 MessageBox，无新增运行时安装包。

合成示例（不代表现场 AccessClient 格式）：

```text
argument_count=9
argv[0] = /T
argv[1] = /SSH2
argv[2] = /L
argv[3] = <redacted>
argv[4] = /PASSWORD
argv[5] = <redacted>
argv[6] = /P
argv[7] = <redacted>
argv[8] = <redacted>
```

## 验证

- PASS：`cargo test -p unfour-app launch --offline`，11 项。覆盖 Xshell 原有解析、
  OAuth 委派、临时请求、脱敏结构、附带值与类似开关的秘密、URI/控制字符、
  非法 Unicode、大小限制、独立文件与保存失败。
- PASS：`node --test scripts/bastion/launch-shape.test.mjs`，原有 15 项合成夹具。
- PASS：Rust 格式检查、Node 脚本语法检查、`git diff --check`。
- PASS：Windows x64 test/standard 发布构建，前端 TypeScript/Vite 构建、MCP sidecar、
  原生 SSH 与本机回显服务均构建完成。构建元数据导出入口正常。
- PASS：实际 `unfour.exe` 接收合成 SecureCRT 参数后生成脱敏报告，窗口被关闭后
  退出码为 2。报告中只有预期的开关、索引与占位符。
- PASS：`verify-windows-package.mjs` 在隔离数据目录及阻断外部 HTTP(S) 的环境中
  验证两次 Xshell 会话的冷/热启动、主机指纹、真实回环 SSH 回显、PTY 尺寸、关闭、
  导航和左侧列表。热态收到不支持的 SecureCRT 调用时原会话保持正常，认证次数仍为 2。
  保存连接/主机信任/终端历史/任务/活动/outbox 对应记录为零，隔离目录中没有测试凭据。
- NOT VERIFIED：诊断弹窗视觉截图。首次窗口在抓取前已关闭，后续 Computer Use
  未获准访问 Unfour。已核对窗口标题、报告文件和第一次退出码，未据此声称视觉验收通过。
- NOT RUN：ESLint 配置排除此 `.mjs` 验收脚本；Node 语法检查和实际执行通过。

本次源自 main `0871e9327c5c19704caca81180d7e2cf19b189ff`，在
`codex/bastion-launch-diagnostics` 本地分支开发，未提交或推送。构建元数据标记
`-dirty`，包中另附源码快照摘要和文件 SHA-256。用户原有的未跟踪规划文档未修改。

真实 scrt 参数、兼容解析与客户环境验收仍待执行。
