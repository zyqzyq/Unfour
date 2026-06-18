import { Select } from '@unfour/ui';

const dbOptions = [
  { value: 'postgresql', label: 'PostgreSQL' },
  { value: 'mysql', label: 'MySQL' },
  { value: 'sqlite', label: 'SQLite' },
];

export const WithOptions = () => (
  <div style={{ padding: '16px', width: 280 }}>
    <Select options={dbOptions} defaultValue="postgresql" />
  </div>
);
export const Disabled = () => (
  <div style={{ padding: '16px', width: 280 }}>
    <Select options={dbOptions} defaultValue="mysql" disabled />
  </div>
);
