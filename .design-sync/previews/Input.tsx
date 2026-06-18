import { Input } from '@unfour/ui';

export const Default = () => (
  <div style={{ padding: '16px', width: 280 }}>
    <Input placeholder="Search…" />
  </div>
);
export const WithValue = () => (
  <div style={{ padding: '16px', width: 280 }}>
    <Input defaultValue="john@example.com" />
  </div>
);
export const Disabled = () => (
  <div style={{ padding: '16px', width: 280 }}>
    <Input placeholder="Not editable" disabled />
  </div>
);
