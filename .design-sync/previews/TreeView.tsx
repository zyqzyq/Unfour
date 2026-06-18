import { TreeView } from '@unfour/ui';
import { Folder, FileText, Database } from 'lucide-react';
import type { TreeViewItem } from '@unfour/ui';

const items: TreeViewItem[] = [
  {
    id: 'src', label: 'src', icon: <Folder size={13} />,
    children: [
      { id: 'app', label: 'App.tsx', icon: <FileText size={13} /> },
      { id: 'main', label: 'main.ts', icon: <FileText size={13} /> },
      {
        id: 'components', label: 'components', icon: <Folder size={13} />,
        children: [
          { id: 'btn', label: 'Button.tsx', icon: <FileText size={13} /> },
        ],
      },
    ],
  },
  { id: 'public', label: 'public', icon: <Folder size={13} /> },
  { id: 'package', label: 'package.json', icon: <Database size={13} /> },
];

export const Default = () => (
  <div style={{ padding: '8px', width: 240 }}>
    <TreeView
      items={items}
      defaultExpandedIds={['src', 'components']}
      selectedId="app"
      onSelect={() => {}}
    />
  </div>
);
export const Collapsed = () => (
  <div style={{ padding: '8px', width: 240 }}>
    <TreeView items={items} onSelect={() => {}} />
  </div>
);
