import { useEffect, useRef } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

export function TerminalPreview() {
  const hostRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!hostRef.current) {
      return;
    }

    const terminal = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontFamily: "JetBrains Mono, Consolas, monospace",
      fontSize: 13,
      theme: {
        background: "#0d1117",
        foreground: "#d6dee7",
        cursor: "#2dd4bf",
      },
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(hostRef.current);
    fitAddon.fit();
    terminal.write("unfour@workspace:~$ SSH service reserved for russh backend\r\n");
    terminal.write("unfour@workspace:~$ session streaming will use Tauri events\r\n");

    return () => terminal.dispose();
  }, []);

  return (
    <section className="surface-panel min-h-0 rounded-md bg-slate-950 p-2">
      <div className="h-full min-h-[360px] overflow-hidden rounded-md bg-slate-950" ref={hostRef} />
    </section>
  );
}
