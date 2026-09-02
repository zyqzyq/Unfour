# Desktop Anonymous Usage Telemetry

Telemetry is used to understand project health, not to track individual user
behavior. The first version measures active installations, version adoption,
supported operating-system and architecture distribution, release channel, and
distribution type.

## Default and notice

Anonymous usage statistics are enabled by default. On the first entry into the
normal Desktop workbench, Unfour shows a non-blocking notice that explains the
collection and provides **Learn more** and **Turn off** actions. The notice is
recorded locally after its first display and is not shown on later launches.
Test/dev builds have network telemetry disabled and do not show this production
telemetry notice.

The first network attempt waits for a 15-second grace period after that notice.
Turning telemetry off during the grace period cancels the attempt and
immediately suppresses telemetry for the current session. If saving that
preference fails, telemetry remains suppressed for the rest of the session and
does not resume because of an optimistic UI rollback. Later launches do not
repeat the notice or grace period.

Telemetry can be turned off or on at any time in **Settings → Privacy →
Anonymous usage statistics**. Turning it off prevents later `app_active`
attempts. Turning it back on allows an attempt when the current UTC day has not
already recorded a successful one.

## Frequency and UTC gating

Each telemetry installation can successfully send at most one `app_active`
event per UTC calendar day. After a successful 2xx response, Unfour stores:

```text
telemetry.last_successful_active_utc_date = YYYY-MM-DD
```

UTC gives every installation one consistent day boundary without collecting a
timezone or depending on a local clock's timezone rules. Unfour does not put a
client timestamp in the payload; server-side Analytics Engine time is the event
time.

Network errors and non-2xx responses do not update the stored date. They do not
show an error toast, block startup, disable a feature, or start an aggressive
retry loop. A later normal lifecycle opportunity may try again.

## Pseudonymous installation identifier

The telemetry installation ID is generated only when an event is eligible to
be sent:

```text
32 cryptographically random bytes
→ Base64URL without padding
→ 43 characters
```

It is stored in the operating-system credential store under this independent
named-secret identity:

```text
scope = "telemetry"
key = "anonymous-installation-id"
```

This ID is stable across app restarts, upgrades, GitHub sign-in/sign-out, and
Pro entitlement changes. It is local application configuration and does not
participate in Cloud Sync.

The telemetry ID is separate from the Account installation ID stored under
`scope = "pro-installation"` and `key = "installation-id"`. Telemetry does not
use or send the Account installation ID, Supabase user ID, GitHub ID, device
ID, MAC address, MachineGuid, or a hardware fingerprint. The telemetry crate
does not depend on Account business semantics.

## Endpoint and complete payload

Stable Desktop builds send:

```http
POST https://telemetry.unfour.dev/v1/active
Content-Type: application/json
```

The telemetry client does not follow HTTP redirects. A redirect response is
treated as a failed send.

The complete JSON payload is:

```json
{
  "event": "app_active",
  "installation_id": "<43-char-random-id>",
  "version": "0.9.2",
  "platform": "windows",
  "arch": "x64",
  "channel": "stable",
  "distribution": "standard",
  "schema_version": 1
}
```

| Field | Meaning |
| --- | --- |
| `event` | The only v1 event, always `app_active`. |
| `installation_id` | Pseudonymous random telemetry installation identifier. |
| `version` | The compiled Unfour application version. |
| `platform` | `windows`, `macos`, or `linux`. |
| `arch` | `x64` or `arm64`. |
| `channel` | The authoritative compiled release channel: `stable` or `test`. |
| `distribution` | The authoritative compiled distribution: `standard` or `microsoft-store`. |
| `schema_version` | Payload schema version, currently `1`. |

The endpoint is configured once by the build-time release profile. Stable
builds receive the production endpoint. Test builds, including the default
`pnpm tauri dev` flow, receive no telemetry endpoint and make no telemetry
network request. There is currently no separate Test telemetry endpoint.

## Data not collected by this event

The client payload does not contain:

- Account ID, Supabase user ID, GitHub ID, email, entitlement, or subscription;
- Workspace data or `workspace_id`;
- API requests, SSH data or sessions, Database data or queries, or MCP usage;
- feature usage, clicks, screens, session duration, or user journeys;
- hostname, operating-system username, locale, timezone, country, or a client
  timestamp;
- IP address fields or location derived from an IP address;
- passwords, API tokens, cookies, authorization headers, or other credentials.

Like any HTTPS request, the connection is routed over the network, but Unfour
does not add an IP address to the JSON payload or derive country information in
the Desktop client.

Server-side endpoint and Analytics Engine code is maintained in
[`zyqzyq/Unfour-cf`](https://github.com/zyqzyq/Unfour-cf).
