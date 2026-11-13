import { useEffect, useState } from "react";

/** Refresh selectable items while a picker is active. Never expose an old scope's rows. */
export function useLiveOptions<T>(scope: string, load: (scope: string) => Promise<T[]>, active: boolean) {
  const [result, setResult] = useState<{ scope: string; items: T[]; failed: boolean }>();
  useEffect(() => {
    if (!active) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const items = await load(scope);
        if (alive) setResult({ scope, items, failed: false });
      } catch {
        if (alive) setResult({ scope, items: [], failed: true });
      } finally {
        if (alive) timer = setTimeout(() => void refresh(), 2000);
      }
    }
    void refresh();
    return () => { alive = false; clearTimeout(timer); };
  }, [scope, load, active]);
  return result?.scope === scope ? result : undefined;
}
