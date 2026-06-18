import { DataTable } from '@unfour/ui';
import type { DataTableColumn } from '@unfour/ui';

type FileRow = { id: string; name: string; status: string; size: string };

const columns: DataTableColumn<FileRow>[] = [
  { id: 'name', header: 'Name', cell: (r) => r.name },
  { id: 'status', header: 'Status', cell: (r) => r.status },
  { id: 'size', header: 'Size', align: 'right', cell: (r) => r.size },
];

const rows: FileRow[] = [
  { id: '1', name: 'main.ts', status: 'Staged', size: '4.2 KB' },
  { id: '2', name: 'App.tsx', status: 'Modified', size: '12.8 KB' },
  { id: '3', name: 'styles.css', status: 'Staged', size: '8.1 KB' },
  { id: '4', name: 'package.json', status: 'Untracked', size: '1.4 KB' },
];

export const Basic = () => (
  <div style={{ padding: '12px' }}>
    <DataTable columns={columns} rows={rows} getRowKey={(r) => r.id} />
  </div>
);

export const Empty = () => (
  <div style={{ padding: '12px', width: 400 }}>
    <DataTable
      columns={columns}
      rows={[]}
      getRowKey={(r) => r.id}
      empty={<span>No files changed</span>}
    />
  </div>
);
