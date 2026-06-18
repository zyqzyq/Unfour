import * as React from 'react';
import { ContextMenu, ContextMenuTrigger, ContextMenuContent, ContextMenuItem } from '@unfour/ui';

export const MenuOpen = () => {
  const areaRef = React.useRef<HTMLDivElement>(null);

  React.useLayoutEffect(() => {
    if (areaRef.current) {
      areaRef.current.dispatchEvent(
        new MouseEvent('contextmenu', {
          bubbles: true, cancelable: true,
          clientX: 60, clientY: 50,
        }),
      );
    }
  }, []);

  return (
    <div style={{ height: 200, width: 260, position: 'relative' }}>
      <ContextMenu>
        <ContextMenuTrigger>
          <div
            ref={areaRef}
            style={{
              padding: '10px 14px',
              border: '1px dashed var(--u-color-border)',
              borderRadius: 4,
              color: 'var(--u-color-text-muted)',
              fontSize: 12,
              cursor: 'context-menu',
            }}
          >
            Right-click area
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem>Copy</ContextMenuItem>
          <ContextMenuItem>Paste</ContextMenuItem>
          <ContextMenuItem>Rename</ContextMenuItem>
          <ContextMenuItem disabled>Delete</ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
    </div>
  );
};
