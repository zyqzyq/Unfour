import { StatusBadge } from '@unfour/ui';

export const Neutral = () => <StatusBadge>Unknown</StatusBadge>;
export const Success = () => <StatusBadge tone="success">Active</StatusBadge>;
export const Warning = () => <StatusBadge tone="warning">Pending</StatusBadge>;
export const Danger = () => <StatusBadge tone="danger">Failed</StatusBadge>;
