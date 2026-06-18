import { ErrorState } from '@unfour/ui';

export const Default = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <ErrorState>Failed to connect. Check your credentials and try again.</ErrorState>
  </div>
);
export const Short = () => (
  <div style={{ padding: '16px', width: 360 }}>
    <ErrorState>Connection refused.</ErrorState>
  </div>
);
