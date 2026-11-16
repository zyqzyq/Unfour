# Windows A2 离线测试包

这个 x64 便携包用于验证 Windows 启动参数到 Unfour 临时终端的链路。解压到可写目录，先运行包内的本机回显服务。测试程序使用独立的单实例标识。以下命令适用于 Windows PowerShell，参数均为合成值。

以下预期行为对应当前源码及配套验包脚本。本轮 UX 优化尚未重新交付离线包；
已交付的 scrt 包仍有首次指纹确认和固定标签，复验它请使用
[该包历史记录](bastion-scrt-launch-2026-11-16.md)中的步骤。

1. 在解压目录打开第一个 PowerShell 窗口，运行 `.\bastion-loopback.exe`。记下它打印的随机端口和 SHA-256 主机指纹。服务只监听本机回环地址，十分钟后退出。
2. 在同一目录打开第二个 PowerShell 窗口，执行以下代码。输入上一步的端口。两次调用应使用同一个窗口和同一份参数数组。

   ```powershell
   $portA2 = [int](Read-Host 'Fixture port')
   $rootA2 = Join-Path $env:TEMP ('Unfour-A2-' + [guid]::NewGuid())
   $env:UNFOUR_DATA_DIR = Join-Path $rootA2 'workspace'
   $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $rootA2 'webview'
   $argsA2 = @('-url', "ssh://fixture-user:fixture-grant@127.0.0.1:$portA2", '-newtab', 'Synthetic asset@different.invalid')
   & .\unfour.exe @argsA2
   ```

3. 只有一个工作区时应自动进入 **SSH**，标签显示节点名称和连接地址，首次指纹在内存自动记录，不再需要确认。已有指纹不匹配时仍须拒绝连接。在终端完成下述回显检查后，切到 API，重新执行一次 `& .\unfour.exe @argsA2`。应回到 SSH 并保持相同节点显示，两次均应保留左侧 SSH 连接列表。多个工作区时，在目标工作区手动认领请求。当前优化验证范围见 [交互回归记录](bastion-ux-review-2026-11-16.md)。
4. 在每个临时终端分别输入 `check` 并回车，确认终端显示 `A2_ECHO_OK`。关闭该标签，再验证另一个请求。两个请求完成后，本机服务应各打印两次 `FIXTURE_AUTH_OK`、`FIXTURE_PTY_OK` 和 `FIXTURE_ROUNDTRIP_OK`。关闭 Unfour，用 Ctrl+C 停止本次服务。

当前回显服务还会打印 `FIXTURE_RESIZE <列数> <行数>`。首次连接成功后就应出现该消息。自动验收会对照 xterm 显示的行数检查每个会话。新构建使用同一份源码中的验包脚本、应用和回显服务。复测历史包 `e85a8ea7` 时使用其验收提交 `73a55e95` 中的说明和脚本。

测试包附带应用、MCP sidecar 和独立的合成 SSH 服务。手工复现使用 Windows x64、Microsoft WebView2 Runtime 和 Visual C++ x64 运行库。PE 依赖表显示 sidecar 与合成服务需要 `VCRUNTIME140.dll`。缺少运行库时，由现场按软件安装流程使用 `prerequisites` 目录中的微软签名离线安装程序。校验文件见 `SHA256SUMS.txt`。[WebView2 离线部署说明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution#offline-deployment) · [Microsoft Visual C++ 运行库](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)

现场只确认了 AccessClient 4.6.15 启动 Xshell 7 时的外层命令行结构，以及已有窗口新增标签的现象。支持的 argv 顺序为 `-url <SSH URL> -newtab <单一标签参数>`。本次解析接受 ASCII 域名或标准 IPv4、显式十进制端口、由 ASCII 字母数字和 `-._~` 构成的非空身份及凭据。标签可以含空格、非 ASCII 字符和 `@`。Windows 单实例转发以竖线分隔字段，入口因此拒绝含竖线的参数。未知开关、额外参数和未确认的编码均返回固定错误。

不支持的参数现在会在窗口初始化前显示原生诊断提示，并保存只含参数结构的报告。
SecureCRT (`scrt`) 新增支持现场报告的固定 11 参数格式，范围与验收步骤见
[scrt 离线验收说明](bastion-scrt-launch-2026-11-16.md)。诊断文件的数据边界见
[启动诊断说明](bastion-launch-diagnostics-2026-11-16.md)。

现场仍需确认特殊字符编码、认证机制与凭据有效期、代理生命期和末端路径选择。真实值只留现场。A4 使用经授权的测试资产验证实际网页链路。
