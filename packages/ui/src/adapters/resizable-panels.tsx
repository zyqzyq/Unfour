import * as React from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { cn } from "../utils";

export function ResizableSplitPane({
  children,
  className,
  defaultRatio,
  minPaneSize,
  orientation,
}: {
  children: [React.ReactNode, React.ReactNode];
  className?: string;
  defaultRatio: number;
  minPaneSize: number;
  orientation: "horizontal" | "vertical";
}) {
  const hostRef = React.useRef<HTMLDivElement | null>(null);
  const [totalSize, setTotalSize] = React.useState(0);
  const minSizePercent = totalSize > 0 ? Math.min((minPaneSize / totalSize) * 100, 45) : 10;
  const firstPaneSize = Math.min(Math.max(defaultRatio, minSizePercent), 100 - minSizePercent);
  const secondPaneSize = 100 - firstPaneSize;

  React.useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host || typeof ResizeObserver === "undefined") {
      return;
    }

    const measure = () => {
      const bounds = host.getBoundingClientRect();
      setTotalSize(orientation === "horizontal" ? bounds.width : bounds.height);
    };
    measure();

    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, [orientation]);

  return (
    <Group
      className={cn(
        "flex min-h-0 min-w-0 flex-1",
        orientation === "vertical" && "flex-col",
        className,
      )}
      orientation={orientation}
      elementRef={hostRef}
    >
      <Panel className="flex min-h-0 min-w-0" defaultSize={`${firstPaneSize}%`} minSize={`${minSizePercent}%`}>
        {children[0]}
      </Panel>
      <Separator
        aria-label={`Resize ${orientation === "horizontal" ? "horizontal" : "vertical"} split`}
        className={cn(
          "shrink-0 bg-[var(--u-color-border)] hover:bg-[var(--u-color-focus)]",
          orientation === "horizontal" ? "w-px cursor-col-resize" : "h-px cursor-row-resize",
        )}
      />
      <Panel className="flex min-h-0 min-w-0" defaultSize={`${secondPaneSize}%`} minSize={`${minSizePercent}%`}>
        {children[1]}
      </Panel>
    </Group>
  );
}
