export { Badge } from "./badge";
export {
  ActiveEnvironmentSelect,
  type EnvironmentSelectItem,
} from "./active-environment-select";
export { Button, type ButtonProps } from "./button";
export { ConfirmDialog } from "./confirm-dialog";
export { DataTable, type DataTableColumn, type DataTableSelection } from "./data-table";
export { Dialog, DialogClose, DialogTrigger } from "./dialog-primitives";
export {
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
} from "./dialog";
export { IconButton } from "./icon-button";
export {
  I18nProvider,
  createTranslator,
  defaultLocale,
  getLocaleLabel,
  isSupportedLocale,
  normalizeLocale,
  supportedLocales,
  translate,
  useI18n,
  useT,
  type I18nContextValue,
  type I18nResources,
  type Locale,
  type TFunction,
  type TranslationParams,
  type TranslationTree,
} from "./i18n";
export { Input } from "./input";
export {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "./menus";
export { DropdownMenu, DropdownMenuTrigger } from "./menus-primitives";
export { PopoverContent } from "./popover";
export {
  Popover,
  PopoverAnchor,
  PopoverClose,
  PopoverTrigger,
} from "./popover-primitives";
export {
  SegmentedControl,
  type SegmentedControlOption,
} from "./segmented";
export { Select, type SelectOption } from "./select";
export {
  ActivityBar,
  AppShellFrame,
  BottomPanel,
  CommandPalette,
  GlobalToolbar,
  MainWorkspace,
  RightInspector,
  Sidebar,
  SidebarHeader,
  SidebarRow,
  SidebarSection,
  SplitPane,
  StatusBar,

} from "./shell";
  export { EmptyState, ErrorState, LoadingState } from "./states";
export {
  FeedbackProvider,
  useFeedback,
  useFeedbackErrorHandler,
  extractErrorDetail,
  redactForLog,
  SENSITIVE_KEYS,
  type FeedbackApi,
  type FeedbackOptions,
  type FeedbackTone,
} from "./feedback";
export {
  ThemeProvider,
  useTheme,
  type Theme,
  type ThemeContextValue,
  type ThemeMode,
} from "./theme";
export { initializeTheme } from "./theme-init";
export {
  ConnectionStatus,
  StatusBadge,
  type ConnectionStatusValue,
  type StatusTone,
} from "./status";
export { Tabs, type WorkspaceTab, type TabsAction } from "./tabs";
export { Toolbar, ToolbarGroup } from "./toolbar";
export {
  TreeView,
  type TreeViewDropEvent,
  type TreeViewDropPosition,
  type TreeViewItem,
} from "./tree-view";
export { cn } from "./utils";
export { usePlatform, type Platform } from "./platform";
export { VariableTable, type VariableTableItem } from "./variable-table";
