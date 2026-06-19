# Unfour Interaction Guidelines

## 1. Purpose

本文档定义 Unfour 的交互定位、通用交互模型、核心模块目标形态和分阶段实施边界。

本轮工作的目标不是视觉换肤，也不是一次性重写页面，而是在不改变后端接口、不改变
Command Bus 协议、不删除现有功能的前提下，让 Unfour 从“表单页面集合”演进为成熟的
开发者桌面工作台。

本文档同时记录 2026-06-15 的代码审计结果。文中使用以下术语：

- **Current**：当前代码已经具备的能力。
- **Gap**：当前实现与目标交互模型之间的差距。
- **Target**：后续阶段应达到的交互行为，不代表当前已经实现。

## 2. Product Interaction Positioning

Unfour 的产品交互定位是 **开发者桌面工作台（Developer Desktop Workbench）**。

核心体验原则：

1. **对象优先，而不是表单优先**
   - 用户先在树、标签页或历史记录中定位请求、连接、表、查询和会话。
   - 编辑表单用于修改对象属性，不应长期占据主要导航空间。

2. **工作上下文可保持**
   - 打开的请求、SQL、表数据和终端会话以 Workbench Tabs 表达。
   - 切换模块或资源时，尽量保留未完成输入、选中项、滚动位置和面板尺寸。

3. **主操作明确**
   - API 的主操作是 `Send`。
   - Database 的主操作是 `Run`。
   - SSH 的主操作是 `Connect` 或 `New Session`。
   - 一个操作区域只应突出一个主操作。

4. **状态持续可见**
   - 连接、执行、保存、失败、重连、脏数据等状态不能只依赖瞬时通知。
   - 状态应出现在对象附近、标签页、工具栏或 Status Bar 中。

5. **鼠标和键盘等价**
   - 高频操作必须能通过工具栏、右键菜单和快捷键完成。
   - 键盘焦点、选中状态和激活状态必须可区分。

6. **渐进披露**
   - 主路径保持简洁，低频操作进入更多菜单、Inspector、ContextMenu 或命令面板。
   - 不把所有设置和操作同时平铺在页面中。

7. **桌面密度**
   - 使用紧凑行高、稳定面板和结构边界。
   - 避免营销式卡片、超大标题、过度留白、装饰性阴影和无意义嵌套容器。

## 3. Interaction Audit

### 3.1 Shared UI and App Shell

**Current**

- `packages/ui` 已提供 `TreeView`、`ContextMenu`、`DropdownMenu`、`Tabs`、
  `SplitPane`、`Toolbar`、`DataTable`、状态组件、Dialog 和 Shell 组件。
- `packages/app-shell` 保持为薄组合层，通过 slots 组装全局工具栏、侧栏、主区、
  Inspector、Bottom Panel 和 Status Bar。
- `apps/desktop` 已存在统一 Shell、模块侧栏、全局标签栏、命令面板入口和布局持久化。

**Gap**

- `docs/ui/ui-components.md` 的 Known Gaps 与当前代码不完全同步，仍把若干已存在组件
  描述为待迁移；后续维护时需统一文档事实。
- `TreeView` 有树语义和展开状态，但缺少方向键导航、Home/End、类型搜索、
  roving tabindex、受控展开和加载态节点。
- `ContextMenu` 是自定义实现，支持右键打开和 Escape 关闭，但缺少完整菜单焦点循环、
  键盘打开方式、viewport 边界修正、分组、分隔线、危险项语义和快捷键提示列。
- `Tabs` 与 Shell `TabBar` 是两套相近实现。当前支持选择、关闭和 modified 标记，
  但缺少标签右键菜单、关闭确认、固定标签、重排、恢复关闭标签和统一 dirty 语义。
- `SplitPane` 支持鼠标拖动，但比例是组件本地状态，缺少受控值、双击复位、键盘调整、
  折叠和通用布局持久化契约。
- `DataTable` 当前偏展示型，缺少单元格/行选择、键盘导航、复制、列宽调整、排序、
  右键菜单和大数据虚拟化策略。
- `EmptyState`、`LoadingState`、`ErrorState` 已共享，但没有通用 `SuccessState`、
  inline feedback 和 retry/action 插槽规范。
- Workspace 菜单仍直接使用 Radix 原语，未完全复用 `packages/ui` 的菜单和 Dialog 包装。
- Right Inspector 与非 SSH Bottom Panel 仍是 placeholder，Shell 结构存在但尚未形成
  稳定的模块使用规则。

### 3.2 API Debugger

**Current**

- 已有 Collection 分组、请求编辑区、请求配置 Tabs、响应 Tabs、History 表格、
  Send 主操作、保存/复制/导入/导出动作和响应状态信息。
- 请求区与响应区使用 `SplitPane` 结构，但当前页面调用没有启用 resize。
- 请求状态可表达 new、selected、sending、success、failed、network error 和 timeout。

**Gap**

- 左侧 Collection 是分组列表，不是统一 `TreeView`；History 只是空提示，真实历史位于
  响应区内部。
- 页面只有一个请求编辑上下文，没有 Postman-like Request Tabs。
- Name 和 Folder 表单长期占据请求主区域，使首要操作 `Method + URL + Send` 不够集中。
- 当前状态表示执行结果，不表示请求内容相对已保存版本的 dirty/saved/unsaved 状态。
- 保存成功后没有更新明确的保存基线；新建、历史回放和已保存请求之间的语义不够清楚。
- Params/Headers 的 `KeyValueEditor` 是页面私有实现，没有删除行、拖动、批量编辑、
  重复键提示、键盘增行和行级错误模型。
- Auth 当前主要承载环境变量，不是独立的请求认证模型。
- Collection 节点缺少右键菜单和快捷键入口。
- ResponseTabs 中仍有页面私有 `EmptyState`，与共享状态组件不一致。
- 删除请求当前直接执行，缺少危险操作确认。

### 3.3 Database

**Current**

- 已有连接选择、Schema Tree、SQL Editor、Results/Messages/Logs、表数据、
  Inspector、复制 TSV、导出 CSV、执行确认和数据库状态栏。
- SQL 编辑和查询结果已经形成上下工作区结构。
- Schema 与 Result 已复用 `TreeView`、`Tabs`、`DataTable` 等共享组件。

**Gap**

- 连接编辑表单常驻左栏，导航树和配置表单混在一起，不符合对象优先模型。
- Shell 侧栏与 Database 页面内部左栏重复表达连接/Schema 上下文。
- SQL Tabs 是固定展示项，没有真正的多查询文档生命周期、关闭或 dirty/saved 模型。
- Result Table 只能展示和整表复制，缺少单元格选择、区域复制、列操作和右键菜单。
- Query History 尚未形成可浏览、筛选、重新打开的区域。
- Messages/Logs 是简化文本，缺少按执行记录绑定的时间、状态、影响行数和错误定位。
- Stop 动作目前为空实现；规范不能把它描述成可用能力。
- 删除数据库连接缺少确认。
- Table Inspector 常驻占宽，尚未按选择上下文和窄窗口规则自动折叠。

### 3.4 SSH / Terminal

**Current**

- 已有 SSH Connection Tree、Session Tabs、会话状态、连接/关闭/自动重连、
  Reconnect、Cancel Reconnect、Split、Search、Clear、Export Logs 和 Status Bar。
- Connection Tree 已使用右键菜单，并能显示连接与会话状态。
- 终端日志经过脱敏处理，支持复制与导出。

**Gap**

- `Reconnect` 当前复用新建会话动作，规范上需要区分“恢复当前会话上下文”和
  “基于连接新建会话”。
- Terminal Tabs 缺少右键菜单、关闭其他/右侧标签、复制会话信息等桌面终端常用动作。
- Clear Terminal 菜单项使用了 Copy 图标，动作语义与图标不一致。
- Toolbar 中 Copy Logs 不直接可见，复制日志主要位于 Bottom Panel。
- Connection Tree 中部分菜单项是 disabled 占位项，需要避免让未实现能力看起来可用。
- 关闭仍在连接中的会话、删除连接和清理日志缺少统一确认策略。
- 会话状态分散在树、Tab meta、Toolbar 和 Status Bar，尚未定义状态优先级。
- Split View 有模式切换，但 pane 与 session 的绑定规则、焦点切换和持久化未规范化。

### 3.5 Workspace / Settings

**Current**

- Workspace 支持切换、新建、重命名、删除和布局持久化。
- 删除默认 Workspace 或唯一 Workspace 已被禁止。
- SSH Connection 设置使用共享 Dialog；Workspace Dialog 仍直接组合 Radix 原语。

**Gap**

- 当前没有统一 Settings 页面或 Settings 导航模型。
- Workspace 创建/重命名操作在 mutation 完成前就关闭 Dialog，失败反馈容易丢失。
- Workspace 删除确认没有输入验证或关联资源摘要。
- Workspace、连接设置和模块偏好使用不同 Dialog 结构与反馈方式。
- 全局快捷键、布局偏好、编辑器偏好和模块偏好尚未形成统一的信息架构。

## 4. Common Page Structure

模块页面应组合以下结构，而不是各自创建完整页面 Shell：

```text
Global Toolbar
├─ Sidebar
│  └─ Resource Tree / Navigation
├─ Main Workspace
│  ├─ Workbench Tabs
│  └─ Main Panel
│     ├─ Module Toolbar
│     ├─ Primary Editor / Viewer
│     └─ Optional Bottom Results / Messages
├─ Inspector (optional, contextual)
└─ Status Bar
```

### Sidebar

- 用于模块导航、资源树、收藏、历史入口和小型筛选。
- 不放置完整连接表单、长表单或主编辑器。
- 选择资源默认在当前 Workbench Tab 打开；显式“Open in New Tab”创建新 Tab。
- 折叠后保留模块入口和 tooltip，不保留不可理解的纯状态文本。

### Workbench Tabs

- 表达当前正在处理的工作对象，而不是只表达模块名称。
- API 对象是 Request，Database 对象是 Query/Table，SSH 对象是 Session。
- Shell 可保留模块级入口 Tab；模块内部可以有对象级 Workbench Tabs，但两者职责必须明确，
  避免相邻的两排标签表达同一层级。

### Main Panel

- 承载当前对象的主要编辑、执行和结果查看。
- 主操作靠近主要输入，不能被远距离工具栏或低频设置分隔。
- 复杂页面优先使用可调整 SplitPane，而不是固定三列 grid。

### Inspector

- 展示当前选择对象的元数据、属性、结构或低频配置。
- 没有上下文时自动折叠或显示轻量空态。
- 不复制主操作，不作为错误信息的唯一承载位置。

### Status Bar

- 展示持续状态：Workspace、连接、活动会话、执行状态、行列信息、编码或同步状态。
- 不用于一次性成功消息。
- 状态文本应简短、稳定并可通过 tooltip 查看详情。

## 5. Shared Interaction Specifications

### 5.1 TreeView

**Selection and activation**

- 单击行选择节点；展开箭头只改变展开状态，不隐式执行节点主动作。
- 双击叶节点执行默认打开动作；双击父节点切换展开。
- 选中状态、焦点状态和 hover 状态必须视觉可区分。
- disabled 节点不可选择，必须提供不可用原因的 tooltip 或邻近说明。

**Keyboard**

- `Arrow Up/Down`：移动可见节点焦点。
- `Arrow Right`：展开父节点；已展开时进入第一个子节点。
- `Arrow Left`：折叠父节点；已折叠或叶节点时回到父节点。
- `Home/End`：移动到第一个或最后一个可见节点。
- `Enter`：执行默认打开动作。
- `Space`：选择节点，不触发双击动作。
- `Shift+F10` 或 Menu key：打开 ContextMenu。
- 输入字符时进行同层级或全树的前缀查找。

**State**

- 支持受控 `expandedIds`、`selectedId` 和异步节点 loading/error 状态。
- 刷新树时保留仍然存在节点的展开和选择状态。
- 节点 id 必须稳定，不使用可变数组索引。
- 大树应延迟加载子节点；只有确认存在性能问题时再引入虚拟化。

**Actions**

- 行尾 action 默认在 hover、focus 或选中时显示，但关键状态始终可见。
- 行尾 action 和 ContextMenu 应调用同一业务 handler。
- 不在树行内嵌完整表单。

### 5.2 ContextMenu

- 右键菜单提供对象上下文动作，不应是唯一入口。
- 高频动作顺序：Open/Run/Connect、New/Duplicate、Copy/Export、Refresh、
  Settings、Dangerous Actions。
- 分组间使用 separator；危险操作置于最后一组并使用 danger tone。
- 不显示尚未实现的占位操作；仅在用户能理解不可用原因时使用 disabled。
- 菜单项可显示快捷键提示，但快捷键必须真实可用。
- 菜单支持 Arrow Up/Down、Home/End、Enter、Space、Escape 和字符查找。
- 菜单打开时焦点进入首个可用项，关闭后焦点返回触发对象。
- 菜单必须自动避让窗口边缘和 Status Bar。
- `Shift+F10` 与 Menu key 必须能在键盘焦点对象上打开同一菜单。

### 5.3 Workbench Tabs

**Lifecycle**

- 新建对象创建 `Untitled` Tab；已保存对象使用可识别名称。
- 单击树节点可复用当前 preview Tab；双击或显式 Open in New Tab 固定该 Tab。
- 关闭 dirty Tab 必须确认 Save / Don't Save / Cancel。
- 执行中的 Tab 关闭时，按模块能力提示 Stop and Close 或 Keep Running。

**Visual state**

- Active：当前可见对象。
- Focused：键盘焦点所在 Tab。
- Dirty：标题旁显示稳定圆点，不使用标题前的 `* ` 文本。
- Loading/Running：独立 spinner 或进度点，不与 dirty 标记混用。
- Error/Disconnected：使用状态图标并保留标题可读性。

**Operations**

- 支持关闭、关闭其他、关闭右侧、重新打开已关闭标签。
- 中键关闭和 `Ctrl/Cmd+W` 关闭当前 Tab。
- `Ctrl/Cmd+Tab` 切换最近使用的 Tab。
- 标签溢出时允许横向滚动和溢出列表，不压缩到不可读。
- 后续可增加拖拽重排；第二阶段不以拖拽作为必要条件。

### 5.4 SplitPane

- 支持 horizontal 和 vertical 两种方向。
- 拖动 handle 时使用 pointer capture，避免指针离开 handle 后丢失拖动。
- handle 应有至少 6px 可命中区域，视觉分隔线可保持 1px。
- 支持 min/max size，任何 pane 不得被拖到无法恢复。
- 双击 handle 恢复模块默认比例。
- handle 获得焦点后，方向键按固定步长调整；`Shift` 加速调整。
- 折叠和恢复必须保留上一次非折叠尺寸。
- 比例通过受控 props 暴露，布局持久化由 composition/store 层负责。
- 窄窗口可切换方向或折叠次要 pane，不允许主编辑器低于可用宽度。
- 保持现有 props 兼容；新增受控能力时保留 `defaultRatio` 行为。

### 5.5 Toolbar

- 一个 Toolbar 只突出一个 primary action。
- 左侧放上下文和对象选择，右侧放执行与视图动作。
- 动作顺序稳定，运行期间不移动按钮位置。
- 相反动作使用同一位置切换，例如 Run/Stop、Connect/Disconnect。
- icon-only 动作必须使用 tooltip 和 accessible label。
- 低频动作放入 DropdownMenu；不要用多个同等权重按钮填满工具栏。
- 窄宽度时先隐藏文本标签，再进入 overflow menu；主操作始终可见。
- 执行动作显示 pending 状态并阻止重复提交。
- 成功反馈使用邻近状态或短暂 toast；错误必须保留到用户处理或下一次执行。

### 5.6 KeyValueEditor

- 建议作为 `packages/ui` 的无业务逻辑共享组件，由 feature package 提供数据和校验。
- 每行包含 Enabled、Key、Value、可选 Description/Type、Row Actions。
- 最后一行填写后自动追加空行；也保留明确的 Add Row 操作。
- `Tab` 在单元格间移动；最后一个单元格按 `Tab` 创建新行。
- `Enter` 默认进入编辑或确认当前单元格，不提交整个页面。
- `Ctrl/Cmd+D` 复制当前行；`Delete` 仅在行选择模式删除，编辑输入时保持文本删除语义。
- 支持删除行、清空、批量粘贴和文本模式，但第二阶段可先实现删除与键盘增行。
- 空 Key 的空行不参与发送；非空 Value 配空 Key 时显示行级校验。
- 重复 Key 显示 warning，不擅自合并或覆盖。
- 敏感值必须支持 mask/reveal，复制操作需要明确，日志和历史继续遵守脱敏规则。
- 组件 props 保持通用，不引入 API Header/Auth 等业务概念。

### 5.7 DataTable

**Navigation and selection**

- 单击选择单元格；拖动或 Shift 扩展区域；行头可选择整行。
- 方向键移动 active cell，`Home/End` 在行内移动，`Ctrl/Cmd+Home/End`
  移动到数据边界。
- 双击或 `Enter` 进入编辑仅用于明确可编辑的数据表；查询结果默认只读。
- active cell、selected range 和 hover row 必须可区分。

**Copy and export**

- `Ctrl/Cmd+C` 复制选中单元格或区域，默认使用 TSV。
- 单元格 ContextMenu 提供 Copy Cell、Copy Row、Copy Column Name、
  Copy as TSV/CSV/JSON。
- `NULL`、空字符串、binary 和 object 必须有明确且可逆的复制规则。
- 大结果集导出不得阻塞主 UI；若后端暂不支持，保留当前前端导出但显示范围说明。

**Columns and performance**

- 支持列宽调整、双击 auto-fit、固定表头和横向滚动。
- 排序或筛选只有在数据源语义明确时启用，必须标明作用于当前页还是全量数据。
- 大数据量达到可测性能瓶颈后再引入虚拟化，不提前增加大型表格依赖。
- 保持现有 `DataTable` props 兼容，通过可选 props 增加 selection、contextMenu 和 resize。

### 5.8 Empty, Loading, Error, Success

**Empty**

- 区分“无对象”“无选择”“结果为空”和“筛选无匹配”。
- 空态说明下一步，并在存在明确主路径时提供一个 action。
- 不用相同的 “No data” 覆盖所有场景。

**Loading**

- 首次加载使用区域 loading state；刷新已有内容时保留内容并显示局部进度。
- 超过短时阈值再显示 spinner，避免快速操作闪烁。
- loading 期间保持布局稳定，不用整页替换导致焦点丢失。

**Error**

- 错误出现在失败操作附近，并提供 Retry、Open Settings 或 Copy Details 等恢复动作。
- 网络错误、超时、权限错误、输入错误和业务拒绝应使用不同文案。
- 错误不能只显示在 Status Bar 或瞬时 toast。

**Success**

- 保存、复制、刷新等成功使用短暂 inline feedback 或 toast。
- 创建成功后聚焦新对象；执行成功后聚焦结果，而不是弹出阻塞 Dialog。
- 持续成功状态，例如 Connected/Saved，使用稳定 badge 或 status text。

### 5.9 Dirty, Saved, Unsaved

三个状态必须按对象内容与持久化基线定义：

- **Saved**：当前内容与最近一次成功保存的服务端/持久化快照一致。
- **Dirty**：已保存对象被修改，当前内容与保存基线不同。
- **Unsaved**：对象从未保存，或由 History/临时结果创建且没有持久化 id。

规则：

- Send、Run 和 Connect 不等于 Save。
- 保存请求开始时状态为 Saving；只有 mutation 成功后才更新保存基线。
- 保存失败后保持 Dirty/Unsaved，并显示可重试错误。
- dirty 比较应基于规范化业务输入，不基于 React state 对象引用。
- 切换 Tab 保留 dirty 内容。
- 关闭、切换 Workspace、删除来源对象或退出应用前检查 dirty 对象。
- 自动保存只适用于低风险布局与偏好；请求、SQL 和连接配置默认显式保存。
- Tab、Toolbar 和关闭确认使用相同的 dirty 状态来源。

### 5.10 Keyboard Shortcuts

快捷键由 Shell 注册全局命令，由激活模块注册上下文命令。输入框、Monaco 和 xterm
优先处理文本编辑快捷键，模块不得无条件拦截。

| Action | Windows/Linux | macOS | Scope |
| --- | --- | --- | --- |
| Command Palette | `Ctrl+Shift+P` | `Cmd+Shift+P` | Global |
| Quick Open | `Ctrl+P` | `Cmd+P` | Global |
| Save | `Ctrl+S` | `Cmd+S` | Active object |
| Close Tab | `Ctrl+W` | `Cmd+W` | Active tab |
| Reopen Closed Tab | `Ctrl+Shift+T` | `Cmd+Shift+T` | Workbench |
| Next Recent Tab | `Ctrl+Tab` | `Ctrl+Tab` | Workbench |
| Toggle Sidebar | `Ctrl+B` | `Cmd+B` | Global |
| Toggle Bottom Panel | `Ctrl+J` | `Cmd+J` | Global |
| Focus Search | `Ctrl+F` | `Cmd+F` | Active panel |
| Send API Request | `Ctrl+Enter` | `Cmd+Enter` | API |
| Run SQL | `Ctrl+Enter` | `Cmd+Enter` | Database |
| Run Selected SQL | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` | Database |
| New Terminal Session | `` Ctrl+Shift+` `` | `` Cmd+Shift+` `` | SSH |
| Clear Terminal | `Ctrl+L` | `Ctrl+L` | Terminal, when xterm permits |
| Open Context Menu | `Shift+F10` | `Shift+F10` | Focused object |

实现规则：

- 快捷键必须集中注册、可查询并显示在 Command Palette 或菜单中。
- 不重复定义互相冲突的 window-level listener。
- 系统保留快捷键和 Monaco/xterm 内建快捷键优先。
- 按键绑定后续应可配置；第一轮实现可固定，但命令 id 必须稳定。
- 菜单显示的平台快捷键文案应按当前操作系统转换。

### 5.11 Dangerous Operation Confirmation

危险程度分三级：

1. **可撤销或低影响**
   - 例如清空筛选、关闭无修改 Tab。
   - 直接执行，并提供 Undo 或可恢复路径。

2. **局部且不可撤销**
   - 例如删除已保存 Request、删除 Connection、关闭活动 SSH Session。
   - 使用确认 Dialog，明确对象名称和影响。

3. **高影响**
   - 例如删除 Workspace、执行 mutation SQL、信任发生变化的 host key。
   - 显示影响摘要、目标环境/连接、不可逆说明。
   - Workspace 删除建议要求输入名称；mutation SQL 保持现有后端确认协议。

通用规则：

- 危险按钮使用 danger tone，默认焦点放在 Cancel。
- `Enter` 不应意外确认高影响操作；需要显式聚焦危险按钮。
- Dialog pending 时禁止重复提交和关闭导致状态丢失。
- mutation 成功后再关闭 Dialog；失败时保留输入和错误。
- 不使用浏览器原生 `window.confirm`。
- 不改变现有后端安全判断，前端确认只是附加交互层。

## 6. API Debugger: Postman-like Target

### 6.1 Target Structure

```text
Sidebar
├─ Collections Tree
│  ├─ Collection
│  ├─ Folder
│  └─ Request
├─ Environments
└─ History Tree

Main Workspace
├─ Request Tabs
├─ Method + URL + Send + Save
├─ Params | Auth | Headers | Body
└─ Response
   ├─ Body | Headers | Cookies | Timing
   └─ Status | Duration | Size
```

### 6.2 Collection / History Tree

- Collections、Folders 和 Requests 使用统一 TreeView。
- Request 节点展示 Method、Name 和 dirty/open 状态，不在窄侧栏同时展示完整 URL。
- History 按 Today、Yesterday、Previous 7 Days 或日期分组。
- 单击历史记录打开 Unsaved preview；Save 后进入 Collection。
- 搜索作用于 Name、Method 和 URL，筛选时保留树路径提示。

### 6.3 Request Tabs

- 每个打开的 Request 拥有独立输入、response、scroll 和 active sub-tab 状态。
- 新请求标题为 `Untitled Request`。
- 已保存请求显示名称；同名时可在 tooltip 中显示 folder path。
- dirty、saving、sending 和 error 使用独立状态，不互相覆盖。

### 6.4 Primary Request Bar

- 第一视觉层只包含 Method、URL、Send、Save 和 More。
- Method 固定紧凑宽度；URL 占剩余空间；Send 始终可见。
- Name、Folder 和描述移动到 Save Dialog 或 Inspector，不长期占据请求主区域。
- `Ctrl/Cmd+Enter` Send，`Ctrl/Cmd+S` Save。
- Send 期间按钮切换为 Cancel 仅在现有后端支持取消后实现；当前阶段只显示 Sending 并防重复。

### 6.5 Request Configuration Tabs

- 顺序统一为 Params、Auth、Headers、Body。
- Tab 显示启用项数量和错误/警告点。
- Params 与 Headers 复用共享 KeyValueEditor。
- Auth 由 API feature package 组合，敏感值继续使用 credential/environment reference。
- Body 保留 Monaco；格式化、语言选择等能力必须在不改请求协议的前提下渐进增加。

### 6.6 Response Area

- Response 区固定提供 Body、Headers、Cookies、Timing。
- 顶部持续显示 HTTP Status、Duration 和 Size。
- Body 支持 JSON 格式化、纯文本 fallback、长响应提示和 Copy。
- 发送错误显示在 Response 区，不用失败 badge 替代错误详情。
- History 从 Response 的一级切换移回 Sidebar；Response 区只表达当前执行结果。

### 6.7 Save State

- 选择已保存请求后建立 normalized snapshot。
- 任一请求字段变化进入 Dirty。
- History replay 和 New Request 为 Unsaved。
- Save 成功进入 Saved，失败保持 Dirty/Unsaved。
- 删除 dirty 请求前同时说明“删除已保存版本”和“丢弃当前修改”的影响。

### 6.8 Context Menu and Shortcuts

Collection/Request ContextMenu：

- Open
- Open in New Tab
- Send
- Rename
- Duplicate
- Copy URL
- Export
- Delete

History ContextMenu：

- Open
- Open in New Tab
- Save as Request
- Copy URL
- Delete from History（仅在已有后端能力后启用）

建议快捷键：

- `Ctrl/Cmd+N`：New Request
- `Ctrl/Cmd+Enter`：Send
- `Ctrl/Cmd+S`：Save
- `Ctrl/Cmd+Shift+S`：Save As
- `Ctrl/Cmd+W`：Close Request Tab

## 7. Database: TablePlus/DataGrip-like Target

### 7.1 Target Structure

```text
Sidebar
└─ Connection / Database / Schema / Table Tree

Main Workspace
├─ SQL Editor Tabs / Table Tabs
├─ Editor or Result Table
└─ Results | Messages | Query History

Inspector
└─ Columns | Indexes | Constraints | Properties | DDL
```

### 7.2 Connection / Schema / Table Tree

- 侧栏只保留对象树和小型工具栏。
- 连接设置移动到 Dialog 或 Inspector。
- 节点层级按实际后端数据表达，不展示伪造的固定 public/schema 节点。
- 双击连接执行 Connect 或展开；双击表打开 Table Data Tab。
- Refresh 应作用于当前节点范围并保留展开状态。
- 树节点显示 connecting、connected、failed 和 stale 状态。

### 7.3 SQL Editor Tabs

- New Query 创建独立 Tab，绑定 connection id 和 SQL draft。
- SQL 修改后进入 Dirty；保存查询能力未提供前使用 Unsaved，不伪装为 Saved。
- Tab 标题默认为 `Query 1`，可从首条语句推断副标题，但不自动改持久化名称。
- `Ctrl/Cmd+Enter` 执行当前语句或全文，`Ctrl/Cmd+Shift+Enter` 执行选区。
- 执行结果与发起执行的 Tab 绑定，切换 Tab 不覆盖其他查询结果。

### 7.4 Result Table

- 默认只读，单元格选中后可复制。
- 支持 Copy Cell、Copy Row、Copy Column、Copy Selection、Copy as TSV/CSV/JSON。
- Toolbar 提供 Refresh、Export、row count、duration。
- 表数据浏览支持分页状态、只读状态和当前 table name。
- 刷新保持列宽、选中单元格和滚动位置；数据消失时清理失效选择。

### 7.5 Messages and Query History

- Messages 展示执行时间、连接、分类、影响行数、warning 和错误详情。
- Query History 按 Workspace 保存现有可用记录；没有后端历史能力前可先记录当前会话内历史，
  但必须明确不持久化。
- 历史项支持重新打开到新 SQL Tab，不直接覆盖当前 dirty SQL。
- Logs 只用于诊断信息，不与用户可读 Messages 重复。

### 7.6 Context Menu

Connection：

- Connect / Disconnect
- New Query
- Refresh
- Edit Connection
- Copy Connection Name
- Delete Connection

Schema/Table：

- Open Data
- Open Structure
- New Query for Table
- Copy Name
- Copy Qualified Name
- Refresh

Result Cell：

- Copy Cell
- Copy Row
- Copy Column Name
- Copy Selection
- Copy as
- Filter by Value（只有在查询模型明确后实现）

所有 mutation、drop、truncate 或 delete 类操作继续遵守后端安全分类和确认协议。

## 8. SSH: VS Code Terminal-like Target

### 8.1 SSH Connection Tree

- Connections 和 Active Sessions 分组展示。
- Connection 节点提供 Connect、Open in New Tab、Open in Split、Edit、Copy SSH Command、
  Delete。
- Session 节点提供 Focus、Reconnect、Copy Session Info、Close。
- 连接状态优先级统一为 failed > reconnecting > degraded > connected > disconnected。

### 8.2 Terminal Tabs

- 每个 Session 对应一个 Tab，标题优先使用 connection name。
- Tab meta 只显示最重要状态；详细 endpoint、PTY size 和 auth 放 Status Bar/tooltip。
- Tab ContextMenu 提供 Close、Close Others、Close to the Right、Reconnect、
  Open in Split、Copy Session Info。
- 关闭 connected session 需要确认；已 closed/failed session 可直接关闭 Tab。

### 8.3 Session Status and Reconnect

- Connecting、Connected、Degraded、Reconnecting、Disconnected、Failed、Closed
  使用统一文案和 tone。
- Reconnect 表示针对当前 Session/Connection 恢复连接；New Session 始终新建会话。
- 自动重连时显示 attempt 和可见的 Cancel Reconnect。
- 重连失败保留终端输出和日志，不用空态替换现有内容。

### 8.4 Terminal Actions

- Copy Logs：复制经过脱敏的当前会话日志，并显示 Copied/Failed 反馈。
- Clear Terminal：清空当前视图缓冲，不伪装为删除持久化历史。
- Export Logs：导出经过脱敏的当前会话日志。
- Close Session：关闭后端会话并保留 closed 状态，随后由用户关闭 Tab。
- Search：继续使用 `Ctrl/Cmd+F`，Escape 关闭搜索并恢复终端焦点。
- Split：明确每个 pane 绑定的 session，焦点 pane 决定 Toolbar 动作目标。

## 9. Workspace and Settings Target

- Workspace 菜单只负责切换和少量管理入口。
- New/Rename/Delete Workspace 使用统一 shared Dialog 结构。
- mutation 成功后再关闭 Dialog；失败时保留输入和错误。
- 删除 Workspace 显示关联 API Requests、Database Connections 和 SSH Connections 的影响摘要；
  若后端暂不提供计数，至少显示 Workspace 名称和本地删除后果。
- Settings 目标信息架构：
  - General
  - Appearance
  - Keyboard Shortcuts
  - Editor
  - Terminal
  - Database
  - API
  - Credentials
- Settings 属于低频配置，不占用主工作区默认首屏。
- feature-specific settings 仍由 feature package 拥有，Shell 只提供挂载和导航。

## 10. Component Ownership and Compatibility

- 通用、无业务逻辑且至少有两个明确消费者的组件优先放入 `packages/ui`。
- API 请求状态、SQL 执行状态、SSH Session 状态继续由各 feature package 拥有。
- `packages/app-shell` 只负责全局组合、mount points 和 shell-level 状态。
- feature package 不依赖 `packages/app-shell`。
- 新增共享组件能力优先使用可选 props，保持现有调用兼容。
- 不为交互优化重写 command-client、Tauri commands 或 Rust Command Bus。
- 不引入大型 UI 框架或大型 Data Grid；优先扩展现有组件与 Radix 原语。
- 未确认用途的现有动作和状态不因视觉整理而删除。

## 11. Phased Implementation Plan

### Phase 1: Interaction Audit and Guidelines

**Scope**

- 扫描 `packages/ui`、`packages/app-shell`、API Debugger、Database、Terminal、
  Workspace/Settings 和 `docs/ui`。
- 建立本文件，定义通用交互模型、模块目标和后续边界。
- 记录文档与代码事实不一致项。

**Out of scope**

- 不修改页面结构。
- 不新增共享组件。
- 不修改业务状态、后端接口或 Command Bus。

**Risks**

- 文档目标超出当前后端能力。
- 把 disabled placeholder 误认为已有功能。
- 不同模块对 Tab、dirty 和状态术语理解不一致。

**Verification**

- 对照代码确认每条 Current/Gap 描述。
- 搜索文档中的占位标记和模糊占位语句。
- 运行 `git diff --check`。
- 人工审阅模块边界和业务逻辑非目标声明。

### Phase 2: Shared Interaction Components

**Scope**

- 在保持 props 兼容的前提下增强 `TreeView`、`ContextMenu`、`Tabs`、`SplitPane`、
  `Toolbar`、`DataTable` 和状态组件。
- 新增无业务逻辑的 `KeyValueEditor`，前提是 API 与第二个明确消费者确认。
- 建立 command/shortcut registry 和统一危险确认 Dialog pattern。
- 补齐 shared component tests、keyboard tests 和基础 accessibility tests。

**Likely files**

- `packages/ui/src/tree-view.tsx`
- `packages/ui/src/menus.tsx`
- `packages/ui/src/tabs.tsx`
- `packages/ui/src/shell.tsx`
- `packages/ui/src/data-table.tsx`
- `packages/ui/src/states.tsx`
- `packages/ui/src/dialog.tsx`
- `packages/ui/src/index.ts`
- `apps/desktop/src/components/*` 中仅与全局快捷键和 Shell 组合直接相关的文件

**Risks**

- 改变焦点和事件传播导致现有点击行为回归。
- 新快捷键与 Monaco、xterm 或系统快捷键冲突。
- 受控/非受控 props 迁移破坏现有调用。
- DataTable 能力扩大后产生渲染性能问题。

**Testing**

- 为 TreeView 键盘导航、ContextMenu 焦点返回、Tabs 关闭、SplitPane 边界、
  DataTable 复制和 dirty close confirmation 增加单元/交互测试。
- `pnpm run build`。
- 运行仓库现有前端测试命令。
- 启动本地 app，检查首屏、键盘焦点、窄窗口、菜单边界和 pane resize。
- 运行 Rust 默认验证，确认前端组件改动未影响跨层构建。

### Phase 3: API Debugger Sample Refactor

**Scope**

- 将 API Debugger 作为交互样板页。
- 左侧改为 Collection/History Tree。
- 增加 Request Tabs 和明确的 Saved/Dirty/Unsaved 状态。
- 重组 Method + URL + Send 主操作区。
- 统一 Params/Auth/Headers/Body 与 Response Tabs。
- 将页面私有通用组件迁移或替换为 `packages/ui` 组件。
- 保持 `sendApiRequest`、`saveApiRequest`、history 和 environment 调用不变。

**Likely files**

- `packages/api-client/src/ApiDebuggerPage.tsx`
- `packages/api-client/src/components/ApiCollectionTree.tsx`
- `packages/api-client/src/components/ApiRequestEditor.tsx`
- `packages/api-client/src/components/ApiRequestToolbar.tsx`
- `packages/api-client/src/components/RequestParamsTabs.tsx`
- `packages/api-client/src/components/ResponseTabs.tsx`
- `packages/api-client/src/hooks/useApiLayout.ts`
- `packages/api-client/src/hooks/useApiRequestTabs.ts`
- `packages/api-client/src/model/*`
- 必要的 `packages/ui/src/*` 兼容增强

**Risks**

- 多 Request Tabs 导致 state ownership 和历史回放语义变复杂。
- dirty snapshot 不规范化会产生误报。
- 切换请求时丢失未保存输入或 response。
- Sidebar History 与现有 Response History 重组时出现功能遗漏。

**Testing**

- 新建、打开、编辑、保存、保存失败、发送、发送失败、历史回放、关闭 dirty Tab。
- Collection 节点右键菜单和快捷键。
- Params/Headers 行编辑、敏感值 mask、重复键 warning。
- Response Body/Headers/Cookies/Timing 的空、长内容、错误和成功状态。
- `pnpm run build`、相关单元测试、现有 Rust 验证。
- 启动本地 app，人工检查 API 首屏与完整主路径。

### Phase 4: Database and SSH Refactor

Database 与 SSH 应拆成两个独立、可回滚的实施批次，不在同一提交中混改。

**Database scope**

- 将连接表单移出常驻 Sidebar。
- 统一 Connection/Schema/Table Tree。
- 建立真实 SQL Editor Tabs、Result Table selection/copy、Messages 和 Query History。
- 保持数据库命令、SQL safety classification 和 confirmation 协议不变。

**Database risks**

- SQL draft 与 result 的 Tab 绑定错误。
- 表格选择/复制影响大结果性能。
- Tree 刷新丢失展开状态。
- mutation SQL 前端确认与后端确认重复或冲突。

**Database testing**

- 连接 CRUD、测试连接、Schema 加载、SQL read/mutation、确认、分页浏览、复制和导出。
- 多 Query Tab 的 dirty 保留与结果隔离。
- `pnpm run build`、数据库现有测试、Rust checks/tests。
- 本地 app 检查 TablePlus/DataGrip-like 主路径。

**SSH scope**

- 统一 Connection Tree、Terminal Tabs、Session Status 和 ContextMenu。
- 明确 New Session 与 Reconnect。
- 统一 Copy Logs、Clear Terminal、Export Logs 和 Close Session。
- 定义 split pane focus/session binding。
- 保持 SSH command、event transport、日志脱敏和 host-key trust 逻辑不变。

**SSH risks**

- 关闭/重连竞态导致错误 session 成为 active。
- xterm 键盘事件与全局快捷键冲突。
- split pane resize 触发错误 PTY resize。
- 日志复制绕过现有脱敏。

**SSH testing**

- Connect、host trust、connected、degraded、reconnecting、cancel reconnect、failed、
  close 和 reopen。
- Terminal Tab 切换、关闭确认、搜索、复制脱敏日志、清空、导出和 split。
- `pnpm run build`、terminal state tests、Rust SSH feature check、Rust tests。
- 本地 app 检查 VS Code Terminal-like 主路径。

## 12. Definition of Done for the Interaction Program

- 高频任务可以通过清晰的对象树、Workbench Tabs 和单一主操作完成。
- API、Database 和 SSH 使用一致的基础交互语言，但保留各自工作流特点。
- Saved/Dirty/Unsaved、Loading/Error/Success 和连接状态具有统一语义。
- 树、菜单、标签、分栏和表格可通过键盘使用。
- 危险操作具有与影响等级匹配的确认。
- 页面不再依赖长期常驻的大表单作为主要导航方式。
- 新能力不改变后端接口、Command Bus 协议或安全边界。
- 共享组件保持业务无关，feature packages 保持业务状态所有权。
- 每个实施阶段可以独立构建、测试、人工检查和回滚。
