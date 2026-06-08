# Next Steps

## Recommended: SSH Transport Phase 2

Priority order:

1. **PTY allocation** — Request a PTY on the russh channel after authentication.
2. **stdin/stdout streaming** — Wire russh channel data to the SshService and push output events.
3. **Tauri events** — Emit terminal output as Tauri events for the frontend to consume.
4. **xterm.js integration** — Connect frontend terminal to the Tauri event stream.
5. **Terminal resize** — Forward resize events from xterm.js through Tauri to the russh channel.

## Lower Priority

- **Private-key authentication** — Support SSH key-based auth using russh's `authenticate_publickey`.
- **Host-key UI** — Allow users to view, trust, or reset host-key fingerprints.
- **known_hosts integration** — Import or export fingerprints from/to the system `known_hosts` file.
