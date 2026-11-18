# 堡垒机 TCP / 临时 SSH Task 主程序现场验收

本轮交付只有 Windows x64 `unfour.exe`。保持既有 AccessClient → Unfour 启动方式，
关闭旧进程、备份旧主程序后替换即可。仍依赖现场已有的 WebView2；不附带完整运行时、
安装包、MCP sidecar 或模拟服务。该测试版与上一轮沿用 test channel。

## 现场问题与修复

- 用户确认失败请求发出时选择了临时堡垒机会话，之后改为直连；直连接口正常。
  之前把不同 SSH 通道拒绝都显示成“目标无法到达”，本轮按协议原因分类。能登录终端
  不证明堡垒机允许 TCP 转发，也不证明堡垒机网络可以访问客户端直连可达的地址。
- SSH Task 保存按钮禁用来自名称为空（“未命名任务”为占位提示），现在有明确提示。
  保存无需本地连接；运行目标增加当前工作区已连接的临时节点。

## 现场验证步骤

1. 从网页重新授权启动 Unfour，确认标签上的节点、地址与预期一致。保持父终端打开。
2. 在“能力检测”选择“TCP 转发”，填写 API 的主机/IP 和端口，**不要填写 URL、路径
   或账号**。确认当前为授权测试资产后检测。此步骤仅建连，发送 0 字节应用数据。
3. 复制脱敏报告，记录结果码。禁止转发需核查堡垒机策略；目标连接失败需从堡垒机
   一侧检查地址、DNS、端口和监听服务；不支持通道、资源不足、断线、超时分别处理。
   检测通过后再自行发送已授权的 API 请求；TCP 成功不等于 HTTP 成功。不要自动重试
   会修改远端数据的请求，先确认上一次结果。
4. 打开 SSH Tasks，新建任务，填写名称，默认连接可留空。添加 command 步骤，Linux 目标例如
   `printf 'UNFOUR_TASK_OK\n'`，保存。在运行对话框选当前临时节点并确认运行。
   检查 stdout、stderr、成功状态及非零退出码失败状态；失败/取消时不能误报成功。
5. 如现场允许文件测试，在 Task 中配置 upload/download，明确选择本机和远端测试
   路径，使用无敏感内容的小文件比对；测试后自行清理授权目录中的测试文件。
   此操作会按配置传输文件，区别于只建连的 TCP 检测。
6. 用批准的短时等待命令检查取消：取消后父终端仍可输入。运行中关闭父标签也应停止
   本地执行。已派发命令的取消不能证明远端进程终止，界面应保留结果不确定提示。
7. 检查临时运行标识：不保存默认/最近连接和输入回放，关闭父标签/离开工作区后结果
   清除；重启不能恢复临时目标。断线后的结果可暂留查看，再运行需新授权和重选目标。
8. 顺带检查上一轮体验修复：节点标签、无首次额外信任点击、首次授权后粘贴、光标闪烁。

只带回脱敏报告、通过/失败和固定错误码。不要带回认证材料、原始 argv/URL、远端输出
或客户文件。报告不含 TCP 端点；同类检测再次运行会替换当前显示结果，需要时先复制。

## 范围与剩余现场能力

本轮支持临时目标运行单个 SSH Task；Flow 编排尚未接入临时目标。独立 WinSCP 入口、
再次登录、授权到期、helper/代理退出、真实 Windows 剪贴板授权弹窗仍需现场验收。
本机模拟服务通过不代表客户堡垒机支持 exec、SFTP 或 direct-tcpip。#3/#7 不因本机
验证通过而视为全部完成。

## 构建与验证记录

源码基线为 `10333c2f468f00093190b0b0b66415e5f162501a`，包含前轮体验修复。
本轮能力检测及 TCP/Task 补充仍为未提交改动；以交付目录 `source-snapshot.json`
的逐文件 SHA-256 对应本次程序。没有新增生产依赖；模拟 SFTP fixture 使用项目已有
版本的 `russh-sftp` 作为 CommandBus dev-dependency。Windows 自动填充修复使用
Tauri 现有窗口配置，不增加 COM 依赖或改造启动流程。

| 检查 | 结果 |
| --- | --- |
| SSH engine native 全量 | PASS，188 项；命令、300 KB 上传下载、取消/晚到通道/父关闭、零运行记录写入 |
| SSH engine 默认功能 | PASS，123 项；最终 CommandBus 非 native `cargo check` PASS |
| CommandBus native 临时链路 | PASS，7 项；执行 guard、无保存连接运行、取消/清理不写活动，普通保存目标转发不回归 |
| Task 变量上下文 | PASS，9 项；临时目标也验证输入版本、秘密与工作区隔离 |
| AppError 契约 | PASS，7 项；固定转发错误码经过序列化仍保留，任意附加远端文本不识别为稳定码 |
| 前端整组 | PASS，58 文件/503 项；SSH、检测 adapter、i18n 和 API 状态分类 |
| 最终异步竞争修复 | PASS，新增 8 项加已有 Session 55 项，共 63 项 |
| 自动填充修复 | PASS，Task 运行普通/敏感参数均明确关闭浏览器自动填充；组件与 SecretInput 定向 19 项 |
| API hook 回归 | PASS，4 文件/31 项 |
| 格式与静态检查 | PASS，相关 Rust fmt、TypeScript、ESLint 无错误、diff 检查；ESLint 仍有复杂度/行数提示 |
| 独立 review | PASS；删除运行中定义、临时清理写活动及四类 UI 迟到请求问题已修复并复核 |

过程中的 CommandBus 首轮 2 项失败是断言尚未更新：拒绝码由泛化的目标不可达到
`SSH_TUNNEL_FORWARD_PROHIBITED`，新增 fixture 成功状态应为 `success`。修正断言后
7 项通过。最初上下文测试过滤器匹配 0 项，改用正确模块过滤器后实际执行 9 项通过。
真实客户网络未被访问，用户失败的 POST 未被重放。

实际 EXE 第一轮验包还发现 WebView2 的 `Web Data/autofill` 表记住了合成运行参数
`fixture-task-ok` / `fixture-task-wait`，而应用 SQLite、运行表和日志没有保存它们。
候选 `ec441f8f…9a56fb7a` 因此被拒绝交付。已为所有 Task 运行参数输入加上
`autoComplete="off"`，保留失败现场和 `qa/autofill-investigation.json`，重新构建后必须
同时通过整目录哨兵扫描和只读 autofill 表零记录检查。没有删除旧 profile 来绕过验证。

第二个候选 `386a9462…546441ad` 的基础 autofill 表已为零，但整文件扫描仍检出值。
进一步只读检查确认 `autofill_edge_field_values` 保存了两次 Task 参数和两种 TCP 目标，
均为实际行而非删除残留。该候选同样拒绝交付，证据为 `qa/edge-autofill-investigation.json`。
最终修复在主窗口配置 `generalAutofillEnabled: false`，由 Wry 在首次导航前关闭
WebView2 `IsGeneralAutofillEnabled`，并保留输入框的 `autoComplete="off"`。
密码自动保存沿用 WebView2 官方默认关闭策略，不增加额外启动钩子。
[微软接口契约](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2settings4?view=webview2-1.0.2849.39)
说明该设置控制包括任意输入在内的表单信息保存。实包验证仍以两表和原始字节扫描
为准，不用接口设置成功代替存储证据。

最终交付目录：`D:\project\Unfour\target\bastion-task-tcp\20261116-175016`。

| 交付 | 内容 |
| --- | --- |
| [unfour.exe](../../target/bastion-task-tcp/20261116-175016/delivery/unfour.exe) | 73,700,864 字节，约 70.3 MiB；delivery 目录只有该主程序 |
| SHA-256 | `1a313c69332724145e0fd61bf6aa93b29bd19f52fbe4236c11760f1ff05a9eca` |
| 构建标识 | `0.14.0` / `test` / `standard` / `10333c2f468f00093190b0b0b66415e5f162501a-dirty` |
| [校验文件](../../target/bastion-task-tcp/20261116-175016/unfour.exe.sha256) | 包外保留，方便现场比对 |
| [源码摘要](../../target/bastion-task-tcp/20261116-175016/source-snapshot.json) | 69 项改动源码/测试输入；文档不进入二进制快照 |
| [验证摘要](../../target/bastion-task-tcp/20261116-175016/qa/verification.json) | 构建、回归、实包结果与边界 |

最终构建日志为 `qa/build-native-autofill-fix.log`，两组实际程序验收日志为
`qa/windows-xshell.log` / `qa/windows-scrt.log`。最终 hash 在两组验证后复核一致。

| 实际交付 EXE 检查 | 结果与范围 |
| --- | --- |
| 启动兼容 | PASS，Xshell 冷/热 + scrt 热；scrt 冷/热 + Xshell 热，共 6 个认证会话 |
| 手动能力检测 | PASS，30 次检测：每会话 exec 0/23、双通道、SFTP 清理、TCP 成功和拒绝；TCP 接收 0 字节，无额外认证，父 PTY 仍可输入 |
| 临时 Task 界面链路 | PASS，无已保存连接仍可保存命名任务、选择临时节点；两次成功与两次取消，stdout/stderr、退出码、结果未知提示、回放为空、内存标识均验证 |
| 零运行数据持久化 | PASS，仅任务定义正常保存；SSH 连接/主机信任/终端历史/命令历史/Task run/运行活动/outbox 均为零，默认与最近连接为空 |
| 浏览器持久化 | PASS，新 profile 的 `autofill`、`autofill_edge_field_values` 均无两种 TCP 目标和两次 Task 参数；整目录 UTF-8/UTF-16 扫描无输入、输出、启动材料及检测标识哨兵 |
| UX 与视觉 | PASS，节点/端点、无额外信任点击、1.4 秒光标、首次剪贴板权限交接单次重试、断线报告保留；主结果面板及能力弹窗完整可读 |
| 最终审查 | 无剩余阻塞项；原生自动填充配置经独立 review，未改变启动流程，无新生产依赖。最终 diff/格式和源摘要核对通过 |

现场真实堡垒机策略、授权/helper 生命周期、真实剪贴板权限弹窗、干净系统运行库安装
仍为 NOT VERIFIED。剪贴板交接由脚本合成，不读取系统剪贴板；应用外部 HTTP(S)
在实包测试中被阻断。单程序验证不包含独立 MCP 功能。

非阻塞观察：Tasks 页底栏目前仍沿用普通终端连接上下文，可能显示“未连接”；
临时任务的目标和结果以运行弹窗及输出面板为准，底栏统一展示留作后续体验项。
本轮未提交、推送或关闭 GitHub issue。Tauri CLI 自动改写的空 features 数组已恢复，
未修改依赖版本或编译特性。保留既有 sqlx lifetime、Vite chunk 和 static VC runtime 提示。
