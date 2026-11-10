# A1：离线观察一次现有 Xshell 测试会话

只观察已获准的一次性会话，保持网页 → AccessClient 4.6.15 → Xshell。
不替换程序、不修改配置／注册表、不复用票据、不试探额外通道、不主动结束代理。
本步骤不验收 Unfour；产物是 A1 的合成结构和时间线。

## 出发前核实工具

2026-11-10 开发机检查：Node v24.15.0、`Get-CimInstance`、
`Get-NetTCPConnection` 可找到；正常本机权限下 `Win32_Process` 类可读取。
沙箱内 CIM 不可访问，说明工具存在不等于现场权限足够。
PATH 中未找到 Procmon/Process Explorer；没有把它们当成已安装依赖。

在离线测试机先运行以下**不含凭据**的命令：

```powershell
Get-Command Get-CimInstance, Get-CimClass, Get-NetTCPConnection
Get-CimClass -ClassName Win32_Process | Select-Object CimClassName
Get-CimInstance Win32_Process -Filter "ProcessId=$PID" |
  Select-Object Name, ProcessId, ParentProcessId, CreationDate
Get-NetTCPConnection -State Listen | Select-Object -First 1 State
# 仅需运行现有合成结构工具时检查；手工重建不需要 Node。
node --version
node --test scripts/bastion/launch-shape.test.mjs
```

内置快照可能漏掉短命进程，且 `CommandLine` 是 **Windows 原始命令行**，
不是末端程序解析后的 argv；不要按空格拆分或声称已经确定参数边界。
字段含义见 [Microsoft Win32_Process](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-process)。

若需捕获短命子进程或文件／注册表访问，先在批准的准备机准备官方
[Process Monitor](https://learn.microsoft.com/en-us/sysinternals/downloads/procmon)，
核对目标 Windows 兼容性、签名、SHA-256、管理员／驱动加载许可，断网演练一次。
当前官方页面列出的 v4.13 支持 Windows 11+ / Server 2019+；不能默认旧现场系统可用。
它可以记录进程命令行、进程树和文件／注册表事件；**不会替你解释应用私有 IPC 或解密票据**。
不要依赖 Sysinternals Live、在线符号下载或现场临时下载。
若工具／权限不可用，使用快照记录可见部分，其余标 `INCONCLUSIVE`，不要修改系统来凑证据。

## 一次会话的操作顺序

1. 确认没有把客户工作会话纳入采集范围。在现场记录测试开始时间、客户端版本和进程基线。
   若用 Procmon，先暂停采集、清空旧事件，限定 AccessClient、其已观察到的 helper 和
   Xshell 的进程／文件／注册表事件；确认采集权限与原始文件保管位置后才开始短时采集。
   显示过滤不等于底层记录已脱敏。不要上传 PML、CSV 或截图。
2. 通过原网页**只发起一次**允许的 Xshell 会话。在本地 PowerShell 查看相关进程：

   ```powershell
   $testProcesses = Get-CimInstance Win32_Process -Filter "Name='LoaderShell.exe' OR Name='Xshell.exe' OR Name='sessproxy-client.exe'"
   $testProcesses | Select-Object Name, ProcessId, ParentProcessId, CreationDate, ExecutablePath
   ```

   名称只是已知候选；从真实进程树补记实际末端与父子关系。已有 Xshell 进程可能接收 IPC，
   无新进程不表示没有交接。父 PID 可能复用，结合创建时间和事件时间判断。
3. **只在现场**查看目标测试进程的 `CommandLine` 或 Procmon 的 Process Create 详情。
   不把真实内容写入命令字面量、shell 历史、聊天、仓库或自动日志。
   记录是 argv、URI、会话文件路径还是疑似 IPC；如有文件路径，用访问事件确认确实被读取，
   只在批准范围内检查对应的测试临时文件，记录字段名和格式，不递归导出 Xshell 会话目录。
   原始命令行不足以证明解析后 argv；边界不确定就标未知，保留原样引号的**合成重建**。
4. 对已确认属于本次测试的 PID，本地运行
   `Get-NetTCPConnection -OwningProcess <测试PID>`，观察监听和已建立连接。
   记录末端是直连远端还是连接 loopback、哪一 helper 持有远端连接、端口是否动态。
   带回时用 `TERMINAL_A / HELPER_A / REMOTE_A / LOCAL_PORT_A` 替换真实 PID、地址、端口。
5. 正常关闭这个测试标签，分别在约 0、5、30 秒查看同组进程／连接；若已无其它标签且允许，
   再正常退出测试专用 Xshell 进程。区分“标签关闭”与“末端进程退出”。停止采集。
   只记录 helper 留存／退出及时间；不杀代理来验证因果，不重放凭据。
6. 在现场手工写一份全新合成记录并逐字段核对，按现场制度清理原始临时采集。
   `launch-shape.mjs` 只接收已准备的 JSON argv 并输出有限词法形状，
   **不会采集进程，也不会从 Windows 原始命令行可靠还原 argv**。

## 只带回这些材料

- `TERMINAL_A ← HELPER_A ← LOADER_A` 等实际程序名／进程关系，别名时间线。
- 原始命令行的合成字符串、若确知则另附解析后合成 argv 数组；注明来源及不确定边界。
  保留开关名、参数数量、引号、空格、分隔符、百分号／Base64 等编码特征；不猜解码次数。
  URI／文件内容／IPC 只带回经许可的字段类别和合成语法。
- 身份／认证字段类别（包括整个票据用户名）、远端／本机代理类别、动态端口关系。
- 正常关闭标签及退出测试末端后的辅助进程行为；无法确定的进程存活依赖单独列出。
- 可取得的路径配置入口／厂商说明和查找优先级证据、服务器版本。没有就写未知。

路径优先级不能由一次启动自然推断；本步骤不授权改注册表做对照实验。
这些材料决定真实入口适配及是否需要兼容启动器，终端成功不证明 exec、SFTP 或多通道能力。
