# Open Issues

## SSH Transport

- **PTY allocation:** Not implemented. The russh channel is connected but no PTY is requested. Terminal I/O streaming depends on this.
- **stdin/stdout streaming:** Not implemented. The russh channel data flow is not wired to the frontend.
- **Tauri event streaming:** Not implemented. Terminal output events need to be pushed to the frontend via Tauri events.
- **Terminal resize:** Not implemented for native sessions. PTY resize requires a connected channel.
- **Private-key authentication:** Not implemented. Only password auth via SecretStore is supported under `ssh-native`.

## Security

- **OS keychain:** The `keyring` crate is used for production but has not been verified on all target platforms (macOS, Windows, Linux).
- **API body redaction:** Request body redaction is not implemented. Users should avoid saving secrets in API bodies.

## General

- **Real SSH connection verification:** The native transport path has not been verified against a live SSH server in this environment. Manual verification is recommended.
