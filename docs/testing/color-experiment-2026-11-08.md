# Unfour 配色实验 · 2026-11-08

> 历史比较记录：后续已采用 C2（C 中性底色＋浅色紫色交互强调；深色保留 C）。
> 临时 URL 配色切换已移除；以下分支状态、预览方式与结论均保留实验当时语境。
> 本文和截图用于历史对比，当前规范见[设计系统](../ui/design-system.md)。

分支：`codex/ui-redesign-experiment`。所有修改仍未提交、未合并。

## 比较入口

[打开 A–D 同场景对比](artifacts/color-experiment-2026-11-08/index.html)。
查看器提供四套原始截图、两方案叠加比较及原图链接。

本轮以现有默认深色模式为主要比较，浅色模式补充检查。
全部截图为 1440×900、1× 像素比例，由运行中的 React 界面生成；没有制作替代实现的效果图。
API 请求、SQL、返回数据、时间、焦点、选中状态及面板分割位置均使用相同测试条件。

| 方案 | 配色方向 | 取舍 |
| --- | --- | --- |
| A · Graphite / Jade | 当前新版石墨灰与玉绿色，完整保留 | 用于判断其他方案是否改善；不顺带修订基线颜色 |
| B · Legacy Blue | 从 Git `36a75353` 的 Catppuccin Latte / Mocha 恢复蓝色主操作、紫色焦点与偏蓝紫底色 | 延续旧版气质；表面明度梯度、辅助文字及状态色适配新布局，没有机械恢复所有旧值 |
| C · Neutral | 灰阶背景与内容区，蓝紫强调集中于操作、选中和焦点 | 色彩干扰较少；产品辨识度更多依靠现有布局和图标 |
| D · Ink / Amber | 墨蓝工作区、金琥珀强调；浅色为冷灰框架与暖纸色画布 | 冷暖对比明显；主色和警告都属暖色，需要结合橙桃色警告底色、原有标签及图标辨认 |

四套均保持成功绿、错误红、警告黄/橙的语义方向。代码语法高亮、终端、徽章、悬停、选中、焦点和边框一起协调。
没有改动布局、字体、字号、间距、尺寸、圆角、阴影结构、图标或业务行为；打包 U 图标保持不变。

## 临时切换

在当前开发地址后添加 `?palette=A`、`?palette=B`、`?palette=C` 或 `?palette=D`，然后刷新。
省略参数或使用无效参数时均为 A。参数只存在于 URL，不保存为正式主题配置。

如需单独启动一个预览，在仓库根目录运行：

```powershell
pnpm --filter @unfour/desktop dev --host 127.0.0.1 --port 1428
```

随后访问 `http://127.0.0.1:1428/?palette=B`，替换最后的字母即可比较。
明暗模式仍通过原来的设置界面切换，沿用 `unfour.theme` 持久化以及跟随系统的机制。
切换配色需要整页重载，请先保存正在编辑的内容；截图查看器可直接比较，不需要重载工作界面。
浏览器预览使用已有 browser adapter，不等同于连接桌面版真实资源。

实现仅在 `packages/ui` 增加三个颜色覆盖文件和一个临时 URL/Monaco 适配文件，
接入现有样式入口与主题初始化。Monaco 同时注册两种明暗配色，已打开的编辑器可以沿用原设置切换主题。
没有新增依赖、正式主题管理页面或存储结构。

## 场景与验证边界

- API：打开保存的 List members 请求，显示查询参数，点击 Send，展示相同的 200 JSON 响应、137ms 和请求历史；URL 保持焦点。
- Database：连接测试资源、打开保存的 SQL，通过 Monaco 编辑后点击 Run All；展示五行相同结果，选中同一单元格并保持焦点。
- SSH：已有终端会话和相同输出内容，抽查终端、导航、标签与状态颜色。
- Flow：配置请求资源、条件节点和布尔输入，保存同一四节点流程，抽查画布、连线、选中节点与检查器。
- API 与 Database 覆盖四套方案的深色和浅色；SSH 与 Flow 本轮只截图检查深色。

截图测试通过 Playwright 在独立浏览器上下文拦截测试 adapter，提供固定请求和查询结果；
没有修改生产 mock、后端、数据库结构、用户工作区或真实服务。
这些证据验证前端展示和操作路径，不验证真实远端 HTTP、数据库引擎、SSH/SFTP 或 Flow 执行。
原生 Tauri 打包运行、高 DPI、多窗口以及所有弹窗/错误场景的逐项截图未验证。
本轮没有重新执行全仓库或 Rust 测试。

## 基线与恢复

开始配色实验前，已保存全部 133 个改版相关未提交/未跟踪文件及其 SHA-256 清单：

- 完整基线快照：`artifacts/color-experiment-2026-11-08/baseline/working-tree.zip`
- 文件清单与哈希：`artifacts/color-experiment-2026-11-08/baseline/manifest.json`
- 相对于 Git HEAD 的基线补丁：`artifacts/color-experiment-2026-11-08/baseline/working-tree.patch`

这些 `baseline/` 文件是当前工作区保留的本地恢复副本，已从版本跟踪中排除；
不属于可从 Git 获取的交付证据。上述相对路径仅供原工作区恢复时定位。

原有 redesign 截图和文档仍在原位置，未覆盖或清理。
基线的 `tokens.css`、`theme.css` 与现在逐字节一致；A 的 Monaco 定义也保持原样。
快速恢复观感只需删除 `palette` 参数或改为 A。
若要恢复代码，先将 ZIP 解压到单独目录并与当前工作区比较，再按需恢复；
ZIP 保存了未跟踪文件，不能仅靠 `git reset` 重建这份基线。

## 检查记录

| 检查 | 结果 |
| --- | --- |
| 生产构建与 TypeScript | PASS：`pnpm run build`（包含 `tsc`）；保留已有大 chunk 提示 |
| 共享 UI 单元测试 | PASS：15 个文件，68 项测试 |
| A–D 浏览器矩阵 | PASS：2 项矩阵测试、8 个独立上下文、24 张原图；最终 23.9 秒 |
| 基线回归 | PASS：3 项，包含明暗数据画布与紧凑窗口保存校验 |
| 布局与字体一致性 | PASS：各场景控件边界、字体及字形、行高、间距、边框宽度、圆角与 A 相同 |
| 已挂载编辑器主题切换、刷新后的设置持久化 | PASS：8 个上下文；无页面运行错误 |
| 实际抽查控件和状态徽章对比度 | PASS：全部 ≥4.5:1；全局最低 4.82:1，来自 A 浅色 |
| 原始 token 配对 | B/C/D PASS；A 存在 9 个基线失败组合，保留并报告 |
| ESLint、共享 token 规则、`git diff --check` | PASS |
| 对比查看器 | PASS：24 张原图加载、场景/主题切换、并排/叠加、左右交换、SSH/Flow 浅色限制 |

原始 token 共检查 248 对，文字阈值 4.5:1、焦点阈值 3:1。
B/C/D 的最低文字对比度分别为 4.72、4.61、4.64；所有焦点配对通过。
A 的 9 个失败组合主要是辅助/弱提示文字与悬停、active、选中底色的组合，
浅色最低 3.79，深色最低 2.92。它们是更广泛的 token 配对检查结果；
本次实际截图所抽查的控件均通过，不代表所有 UI 状态已逐项验证。
按要求不修订 A，避免改变对照基线。

证据：[原始 token 计算](artifacts/color-experiment-2026-11-08/token-contrast.json)、
[深色控件与几何测量](artifacts/color-experiment-2026-11-08/measurements-dark.json)、
[浅色控件与几何测量](artifacts/color-experiment-2026-11-08/measurements-light.json)、
[最终浏览器运行日志](artifacts/color-experiment-2026-11-08/browser-checks.txt)。

实验时的测试代码：`apps/desktop/tests/smoke/palette-experiment.spec.ts` 与同目录的
`palette-experiment-fixtures.ts`。旧矩阵与配色实现现已保存为
[历史代码快照](artifacts/color-accent-2026-11-08/source-archive/)，不再作为当前可运行测试。
下面保留当时的复现命令；当时使用独立 1427 端口，避免干扰原有开发进程：

```text
node node_modules/@playwright/test/cli.js test --config=test-results/ui-redesign-workbench.config.ts palette-experiment.spec.ts --project=chromium --workers=2 --output=test-results/palette-experiment
```

准备截图过程中修正了测试选择器、browser adapter 对 Run All 的测试响应以及
SSH 测试输出的换行，均只影响新测试文件。没有借此改变生产执行或终端行为。
