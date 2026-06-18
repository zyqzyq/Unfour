import { Toolbar, ToolbarGroup, Button, IconButton } from '@unfour/ui';
import { Settings, RefreshCw } from 'lucide-react';

export const Default = () => (
  <div style={{ width: 480 }}>
    <Toolbar>
      <ToolbarGroup>
        <Button size="sm" variant="ghost">Edit</Button>
        <Button size="sm" variant="ghost">View</Button>
        <Button size="sm" variant="ghost">Filter</Button>
      </ToolbarGroup>
      <ToolbarGroup>
        <IconButton label="Refresh"><RefreshCw size={13} /></IconButton>
        <IconButton label="Settings"><Settings size={13} /></IconButton>
      </ToolbarGroup>
    </Toolbar>
  </div>
);
export const Minimal = () => (
  <div style={{ width: 480 }}>
    <Toolbar>
      <ToolbarGroup>
        <Button size="sm">Run Query</Button>
      </ToolbarGroup>
    </Toolbar>
  </div>
);
