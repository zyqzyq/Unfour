# C2：中性背景与紫色交互强调

> 已采用 C2 为默认设计；临时 URL 配色切换已移除，原有明暗主题设置保持不变。
> 以下内容保留本轮比较时的分支状态、预览方式和结论，截图继续用于历史对比。
> 当前规范见[设计系统](../ui/design-system.md)，最终合入验证另行记录。

日期：2026-11-08。分支：`codex/ui-redesign-experiment`。未提交、未合并。

[打开 B / C / C2 同场景对比](artifacts/color-accent-2026-11-08/index.html)。

本轮按用户要求，将紫色从细焦点轮廓扩展到主操作、活动导航、当前标签及选中底色。
保留 C 的中性背景、正文、普通边框、语法文字、语义状态色，以及所有布局、字体、图标和交互。
原 A/B/C/D、C+ 配色和所有此前截图均保留。默认方案仍未更改。

浅色模式仅覆盖四个现有 token：

| Token | C | C2 |
| --- | --- | --- |
| primary | `#4452b4` | `#6e40bd` |
| primary-hover | `#3e49a5` | `#5d32a8` |
| primary-soft | `#e7eafd` | `#eee7fa` |
| focus | `#4452b4` | `#7640cd` |

沿用组件对这些 token 的既有消费关系；编辑器选区等使用 primary-soft 的位置一起变为浅紫。
深色 C2 与原 C 完全一致。主程序打包 U 图标不变。

## 临时预览

开发地址后添加 `?palette=C&accent=violet` 并刷新，在原设置中选择浅色。
原 C 使用 `?palette=C`；原 C+ 仍使用 `?palette=C&focus=violet`。
参数只在 URL 中存在，不写入主题配置；切换前先保存正在编辑的内容。

## 验证与范围

主比较为真实运行的 React 工作台，使用同一套隔离浏览器 fixtures，固定 1440×900。
API 实际点击 Send 并展示相同响应；Database 在 Monaco 编辑后 Run All，显示相同五行结果。
6 张主图、6 张交互区域截图覆盖 B/C/C2；裁切直接来自运行页面，没有重绘或重新调色。

本轮生产构建与 TypeScript、68 项共享 UI 单测、ESLint、共享 token 规则及 diff 检查通过。
颜色检查共 46 对，通过相应阈值：主色对常用表面最低 4.69:1；
白字对主按钮普通/悬停态分别 6.71:1、8.44:1；新选中底色上的文字最低 5.50:1；
不透明焦点对常用表面最低 4.34:1。该结果不等同于全部组件/透明焦点环的完整无障碍认证。

浏览器矩阵检查四个 token 及既有 aliases 的限定变化、三方案的几何与字体一致性，
以及已挂载编辑器的明暗切换和深色 token 等价。结果与截图一并保存。
没有重新验证原生 Tauri、真实远端服务、SSH/Flow 浅色与高 DPI 场景。

证据：[颜色检查](artifacts/color-accent-2026-11-08/contrast.json)、
[构建日志](artifacts/color-accent-2026-11-08/build.txt)、
[单测日志](artifacts/color-accent-2026-11-08/unit-tests.txt)、
[浏览器检查](artifacts/color-accent-2026-11-08/browser-checks.txt)、
[测量结果](artifacts/color-accent-2026-11-08/measurements.json)。

本轮开始前已备份两个接入文件，并记录全部 216 个已有改动文件的哈希，
保存在 `artifacts/color-accent-2026-11-08/baseline/manifest.json`。
该 `baseline/` 目录仅保留为原工作区的本地恢复副本，不纳入 Git 交付。
旧配色实现与比较测试已另存为[历史代码快照](artifacts/color-accent-2026-11-08/source-archive/)，
与原始截图一起保留；当前产品不加载这些实验文件。
