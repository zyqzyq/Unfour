import { EmptyState } from '@unfour/ui';

export const Default = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <EmptyState>No results found. Try adjusting your filters.</EmptyState>
  </div>
);
export const Short = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <EmptyState>No connections yet.</EmptyState>
  </div>
);
