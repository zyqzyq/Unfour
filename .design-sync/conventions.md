# Unfour UI — Design Conventions

Unfour is a dark-mode-first desktop IDE tool (Tauri 2 + React). The component library uses CSS custom properties for all tokens; Tailwind utilities handle layout and spacing only.

## Token Vocabulary

Color tokens follow the `--u-color-*` prefix. Use these — never Tailwind default colors:

| Token | Purpose |
|---|---|
| `--u-color-bg` | App / page background |
| `--u-color-surface` | Card / panel surface |
| `--u-color-border` | All borders |
| `--u-color-text` | Primary text |
| `--u-color-text-muted` | Secondary / placeholder text |
| `--u-color-primary` | Teal primary action color |
| `--u-color-primary-hover` | Primary hover state |
| `--u-color-primary-foreground` | Text on primary backgrounds |

Radius: `--u-radius-sm` · `--u-radius-md` · `--u-radius-lg`

Badge tones: `--u-badge-{neutral,success,warning,danger,info}-{bg,text,ring}`

## Component Usage Pattern

Components are stateless and require no provider wrapper — import and use directly:

```tsx
import { Button, Badge, Input, DataTable } from '@unfour/ui';

// Primary action
<Button>Save</Button>

// Semantic badge
<Badge tone="green">Active</Badge>

// Labelled text field
<Input placeholder="Search…" />

// Data grid (columns + rows required)
<DataTable columns={cols} rows={rows} getRowKey={(r) => r.id} />
```

## Overlay Components

`Dialog`, `DropdownMenu`, and `ContextMenu` render into portals (fixed/absolute position). Always set `defaultOpen` for design-time previews:

```tsx
<Dialog defaultOpen>
  <DialogContent title="…">
    <DialogHeader><DialogTitle>Title</DialogTitle><DialogXClose /></DialogHeader>
    <DialogBody><DialogDescription>…</DialogDescription></DialogBody>
    <DialogFooter><Button variant="secondary">Cancel</Button><Button>Confirm</Button></DialogFooter>
  </DialogContent>
</Dialog>
```

## State Components

`EmptyState`, `LoadingState`, and `ErrorState` accept optional `children` to override the default message text.

## Button Variants

`default` (teal filled) · `secondary` (gray outlined) · `ghost` (transparent) · `outline` (border only)  
Sizes: `default` · `sm` · `icon`
