import { IconButton } from '@unfour/ui';
import { Plus, X, Settings, Search } from 'lucide-react';

export const AddDefault = () => (
  <IconButton label="Add"><Plus size={14} /></IconButton>
);
export const CloseDefault = () => (
  <IconButton label="Close"><X size={14} /></IconButton>
);
export const Compact = () => (
  <IconButton label="Settings" size="compact"><Settings size={12} /></IconButton>
);
export const Disabled = () => (
  <IconButton label="Search" disabled><Search size={14} /></IconButton>
);
