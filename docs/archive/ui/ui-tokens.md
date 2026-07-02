# Unfour UI Tokens

These tokens are the source of truth for the desktop shell and shared UI
components. Implement them as CSS custom properties in the app stylesheet and
consume them through shared components in `packages/ui`.

## CSS Token Names

Use the `--u-*` prefix for new UI tokens.

```css
--u-color-bg
--u-color-surface
--u-color-surface-subtle
--u-color-surface-muted
--u-color-surface-hover
--u-color-surface-active
--u-color-border
--u-color-border-strong
--u-color-text
--u-color-text-muted
--u-color-text-soft
--u-color-primary
--u-color-primary-hover
--u-color-primary-soft
--u-color-danger
--u-color-warning
--u-color-success
--u-color-focus
--u-size-global-toolbar
--u-size-section-toolbar
--u-size-tabbar
--u-size-sidebar-row
--u-size-table-row
--u-size-input
--u-size-button
--u-size-button-compact
--u-size-statusbar
--u-radius-sm
--u-radius-md
--u-radius-lg
```

Legacy aliases such as `--app-bg`, `--panel-bg`, and `--border` may remain
temporarily, but new code must consume `--u-*` tokens.

## Spacing

Use the following spacing scale only:

| Token     | Value |
| --------- | ----: |
| `space-1` |   4px |
| `space-2` |   8px |
| `space-3` |  12px |
| `space-4` |  16px |
| `space-5` |  20px |
| `space-6` |  24px |

Prefer `space-1` to `space-4` for most desktop UI.

Avoid spacing above `space-6` inside workspace pages.

---

## Heights

| Token                    | Value |
| ------------------------ | ----: |
| `toolbar-height`         |  38px |
| `section-toolbar-height` |  34px |
| `tab-height`             |  34px |
| `sidebar-row-height`     |  28px |
| `table-row-height`       |  30px |
| `input-height`           |  32px |
| `button-height`          |  30px |
| `compact-button-height`  |  28px |
| `statusbar-height`       |  24px |

---

## Typography

| Token              | Value |
| ------------------ | ----- |
| `font-ui`          | 13px  |
| `font-small`       | 12px  |
| `font-label`       | 12px  |
| `font-title`       | 14px  |
| `font-code`        | 13px  |
| `line-height-ui`   | 1.4   |
| `line-height-code` | 1.5   |

Use monospace fonts for:

* terminal
* SQL editor
* JSON viewer
* logs
* request payloads
* code snippets

Do not use large title text in workspace pages.

---

## Radius

| Token       | Value |
| ----------- | ----: |
| `radius-sm` |   4px |
| `radius-md` |   6px |
| `radius-lg` |   8px |

Avoid radius values larger than `radius-lg`.

---

## Borders

Use subtle borders to separate structural regions.

Prefer:

* one border between sidebar and main area
* one border between tab bar and content
* one border above bottom panel
* one border between split panes

Avoid:

* borders around every text block
* deeply nested bordered containers
* decorative outlines

---

## Shadows

Use shadows sparingly.

Allowed:

* dropdown menu
* popover
* dialog
* tooltip

Avoid shadows for:

* normal panels
* sidebar
* tab bar
* table rows
* main workspace sections

---

## Color Usage

Use semantic tokens only.

Required semantic tokens:

```text
background
foreground
muted
muted-foreground
panel
panel-hover
panel-active
border
input
primary
primary-foreground
secondary
secondary-foreground
success
warning
danger
info
focus-ring
```

Do not hardcode colors inside feature modules.

Do not introduce arbitrary accent colors per module.

Use color mainly for:

* connection status
* errors
* warnings
* active selection
* primary action
* syntax highlighting

## App Shell Defaults

| Token | Value |
| ----- | ----- |
| `--u-color-bg` | desktop canvas |
| `--u-color-surface` | primary panel surface |
| `--u-color-surface-subtle` | toolbar and sidebar surface |
| `--u-color-surface-muted` | inactive tabs and secondary controls |
| `--u-color-surface-hover` | hover state |
| `--u-color-surface-active` | selected tree row or active tab |
| `--u-color-border` | normal structural border |
| `--u-color-border-strong` | drag handles and active borders |
| `--u-color-text` | normal foreground |
| `--u-color-text-muted` | secondary foreground |
| `--u-color-text-soft` | placeholders and low-emphasis text |

The shell should avoid gradients and shadows. Use borders, active fills, and
small typography changes to communicate hierarchy.
