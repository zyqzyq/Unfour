# A2 Windows 外层启动验证

关联 [A2 #5](https://github.com/zyqzyq/Unfour/issues/5) 与 [A3 #6](https://github.com/zyqzyq/Unfour/issues/6)。基线为已包含 PR #8、#9 的 `72e5652dcf2e240011c3701ea52c5c18c0fe6e76`。

| 证据来源 | 结果 |
| --- | --- |
| 用户现场观察 | AccessClient 4.6.15 打开 Xshell 7，外层进程以 `-url <SSH URL> -newtab <标签>` 启动。已有 Xshell 窗口时新增标签。原始证据来自任务管理器显示的命令行。 |
| Windows 程序实测 | 同一组已拆分的合成参数分别启动冷进程与第二个进程。主进程保留，第二个进程转发后以 0 退出。SSH 界面有两个待认领请求，认领前本机服务收到 0 次认证。 |
| 终端结果 | 两次分别从 UI 认领，在同一 SSH 握手中核对并确认一次性指纹，在真实 xterm 输入 `check`。两个终端均显示远端返回的 `A2_ECHO_OK`。回显服务精确记录 2 次认证、2 个 PTY shell、2 次输入回显，两次标签关闭成功。 |
| 隔离和拒绝 | 每次运行新建工作区数据库和 WebView2 profile。标签是带空格和 `@different.invalid` 的合成值，连接均到达 URL 的回环端口。增加执行开关后程序以 2 退出，stderr 仅有固定错误码。 |
| 持久化检查 | 最终 ZIP 解压实测后以只读方式检查隔离 SQLite。SSH 保存连接、已知主机、终端历史、命令历史、Task run、SSH 活动和 Cloud Sync outbox 均为 0 行。扫描测试 DB、WAL、日志及 WebView2 profile 的 UTF-8/UTF-16LE 数据，未发现本次身份、凭据、URL 或标签哨兵。Windows 原始进程命令行仍对有权限的本机观察工具可见。 |
| A4 待验 | 客户现场的网页 → AccessClient → Unfour，末端路径选择机制、凭据有效期和复用、特殊字符编码、代理生命期及其与末端进程的依赖。 |

## 包与依赖

| 项目 | 值 |
| --- | --- |
| 程序构建提交 | `e85a8ea7ec2cde68b4a28da414fe08f7047dce84` |
| 程序身份 | Windows x64 MSVC release，Unfour 0.14.0，test/standard；由归档内程序的 `--write-build-metadata` 读回核对 |
| ZIP | `Unfour-A2-win-x64-e85a8ea.zip`，297929291 字节，位于本次工作区的 `target/bastion-a2/` |
| ZIP SHA-256 | `36133fb3798f86c8832afc9d7061d8e913e601170f478471bada6f7afa09b94c` |
| 主程序 SHA-256 | `57ed1992549210b5b8542caa048ab099c1b40c8a62dbf36c96b08ddbd0e36c77` |
| 环境 | Windows 10.0.26200 x64，已安装 WebView2 157.0.4341.42 |

归档内的 `SHA256SUMS.txt` 覆盖应用、sidecar、合成回显程序、build metadata、README 和两份运行库安装程序。解压后 7 个文件逐项通过校验。两份运行库均验得有效 Microsoft Corporation Authenticode 签名。

- `prerequisites/MicrosoftEdgeWebView2RuntimeInstallerX64.exe`，214990032 字节，SHA-256 `1be9d2926954d99dd62653a22efe813160fe0c40f1e422b44e6aaf5473f68eec`。来源为[微软 x64 离线安装程序](https://go.microsoft.com/fwlink/p/?LinkId=2124701)。
- `prerequisites/vc_redist.x64.exe`，14.52.37029.0，SHA-256 `9c02c9ed120f671b900e6b9471d890e3b2414117a7e53f4ae2e855466fa2b7c9`。来源为[微软 VC++ x64 运行库](https://aka.ms/vc14/vc_redist.x64.exe)。

实测使用已安装的运行库。缺少运行库的全新离线机器上的安装过程为 NOT RUN。包内 README 给出手工复现命令，开发机可用已安装的 Node 24 和项目 Playwright 运行 `node scripts/bastion/verify-windows-package.mjs <解压目录>`。该脚本只为新建的测试 WebView2 profile 启用本机随机调试端口，认领、信任和输入均操作产品界面。实际测试使用包内绑定 `127.0.0.1:0` 的 SSH 服务。

## 自动化回归

| 命令／检查 | 结果 |
| --- | --- |
| `cargo test -p unfour-app --features ssh-native --lib launch::tests` | 5 PASS，含未挂载 UI 的队列接收、两个独立请求、端点取值、OAuth、未知/歧义开关、非法 Unicode 与安全拒绝 |
| `node --test scripts/bastion/launch-shape.test.mjs` | 15 PASS，包含现场结构的合成重建 |
| `cargo test -p unfour-account callback` | 5 PASS |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib ssh::temporary` | 9 PASS，队列去重、到期、工作区隔离、首次信任／变更和取消竞争 |
| `cargo test -p unfour-command-bus --features ssh-native --lib temporary_ssh_commands` | 4 PASS，回环单通道、PTY/shell 拒绝、认证／通道超时、取消及迟到成功清理 |
| OAuth API、deep link 与 TemporaryTerminalWorkspace 组件测试 | 3 文件、12 PASS |
| 原生 Windows 打包构建 | `pnpm run tauri build:test --no-bundle --target x86_64-pc-windows-msvc --config <仓库绝对路径>/scripts/bastion/offline-tauri.json`，PASS。使用测试包专用单实例标识；嵌入的前端由完整 TypeScript/Vite 构建生成。 |
| 回显程序构建 | `cargo build --locked --release --target x86_64-pc-windows-msvc -p unfour-command-bus --example bastion-loopback`，PASS |

首次解压验收在两个终端均通过后遇到 WebView2 异步删除临时锁文件，扫描器读取该文件返回 ENOENT。扫描器现只忽略已消失的文件，其他读取错误仍使检查失败。同一 ZIP 解压副本复测通过。

最后一次复测给测试子进程和 WebView2 设置只拒绝连接的本机 HTTP(S) 代理，保留回环地址与应用本地页面。两个请求均完成指纹确认和输入回显，存储检查通过。整机断网与干净系统安装仍待离线环境验证。
