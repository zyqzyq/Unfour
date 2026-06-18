import { ConnectionStatus } from '@unfour/ui';

export const Connected = () => <ConnectionStatus status="connected" label="Connected" />;
export const Connecting = () => <ConnectionStatus status="connecting" label="Connecting…" />;
export const Disconnected = () => <ConnectionStatus status="disconnected" label="Disconnected" />;
export const Error = () => <ConnectionStatus status="error" label="Connection failed" />;
