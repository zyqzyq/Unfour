import { LoadingState } from '@unfour/ui';

export const Default = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <LoadingState />
  </div>
);
export const Custom = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <LoadingState>Syncing data…</LoadingState>
  </div>
);
