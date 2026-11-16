# Architecture and semantics

Browser → same-origin Nginx → SvelteKit UI / Rust Axum `/api/v1`. Rust → PostgreSQL. Rust agents send outbound authenticated HTTPS heartbeat/inventory reports. No Node monitoring API or queue dependency.

## Scheduling

One API instance holds PostgreSQL advisory lock 871221 on a dedicated connection. Another instance fails startup. A periodic connection check terminates the process if lock ownership might have been lost. The scheduler atomically advances due timestamps, claims at most CHECK_WORKERS (1–128) per batch, adds 0–2 seconds jitter, awaits a bounded task set, and loops. Each network check has its monitor timeout (1–30s). HTTP body keyword reads stop at 256 KiB. No target redirect or environment HTTP proxy is followed. Addresses are resolved, checked against private policy and pinned to the connection client to reduce DNS rebinding. Private networking is an explicit trusted-admin opt-in. DNS checks intentionally resolve names without contacting their resolved IPs.

A crash after claiming a check delays it until its next due time. No monitor definition is lost. A slow batch applies backpressure and may reduce achieved check frequency. The UI does not claim exact wall-clock scheduling guarantees.

## State and incidents

A monitor begins PENDING. Consecutive observations reach configurable DOWN/UP thresholds (1–10). Opposite observations reset counters. An UP monitor remains UP during its failure debounce; a DOWN monitor stays DOWN during recovery debounce. PAUSED disables checks and writes a state boundary. MAINTENANCE stores samples but suppresses new incidents and notifications. Maintenance ending resets threshold evidence through subsequent observations. UNKNOWN describes lack of evidence and is shown for missing server inventory; a missing agent heartbeat is an actual failure on its configured host/service monitor.

Recording locks the monitor row, inserts its check, updates counters/state, opens/closes its incident and inserts notification jobs in one transaction. A partial unique index permits one open incident per monitor. Notification uniqueness is channel + incident + transition. Failure timeline events are throttled to one per five minutes. Incident durations are wall-clock intervals; annotations never reach public APIs.

## Availability

Availability = confirmed-UP seconds / (confirmed-UP + confirmed-DOWN seconds). Spans use UTC check timestamps with `lead`, a carry-in sample, and the query-window boundary. An observation covers at most twice its check interval, preventing a stopped scheduler from silently accruing uptime. UNKNOWN, PENDING, PAUSED, MAINTENANCE and missing spans are excluded from numerator and denominator. No known coverage → null, rendered `—`. UI reports covered time alongside availability. Availability is a measured ratio, not a guarantee about unobserved intervals. Dashboard aggregate weights monitors by observed duration.

## Agents

Each server has a random 256-bit bearer credential stored only as SHA-256. A credential selects its own server ID server-side; request bodies cannot claim another server. UUID submission keys deduplicate retries for 24 hours. Inventory and version are bounded by body/serialization limits. Inventories are snapshots, never arbitrary executable check descriptions. Agent configuration is operator-owned; PM2 uses fixed `jlist`, explicit homes/binary, five-second timeout, kill-on-drop and output cap. Docker uses Bollard locally. Central service checks match discovered IDs; absence/unhealthy or heartbeat age >60s fail a check. Heartbeats alone do not automatically enroll monitors.

## Alerts

Transactional jobs persist independent of browser clients. The single worker leases due jobs, uses bounded provider deadlines, and records success or eight-attempt exponential retry/failure. A crash between remote acceptance and DB acknowledgement can duplicate an alert. Discord embeds omit targets and secrets. Mentions are off by default and may be enabled per channel for authored message content; interpolated values cannot add pings. HTTP transport uses pinned validated addresses and no redirects. Encryption uses AES-256-GCM, unique random 96-bit nonce, 32-byte environment key; stored value is base64(nonce || ciphertext || tag). Back up the key separately.

## Real-time choice

SSE fits one-way invalidation updates and browser-native reconnection without WebSocket session complexity. Cookie authorization applies on connection; payloads contain resource IDs or invalidation names, no secrets or complete snapshots. Bounded broadcast capacity 128 drops lagged messages. Clients refetch authoritative data after messages and every 30s as fallback. Stream keepalives prevent idle proxy expiry, Nginx buffering is disabled. No replay log is promised. Session revocation affects subsequent API calls immediately; streams revalidate at most fifteen seconds later and only carry non-sensitive invalidations.

## Storage and ownership

SQLx migrations are versioned and applied on startup. Runtime parameterized queries allow offline builds without requiring compile-time database access; integration tests execute SQL against isolated real PostgreSQL databases. One workspace with roles, not multitenancy. Raw checks/auth audit/deliveries are pruned after 90 days; sessions and submission keys have shorter retention. Durable monitor/incident definitions survive process restarts. Retention is currently a fixed documented default.

## Public resources and editorial status

A resource is a public-safe presentation layer over zero or more explicitly public-eligible monitors. Automatic severity aggregates current effective monitor states; unknown coverage is never silently operational. An optional audited override changes presentation, with a reason and optional expiry, while raw observations and incident hysteresis remain authoritative. Availability combines duration-weighted measured coverage of linked monitors. A manually managed resource without monitors has null uptime.

Publication selects pages and safe editorial copy explicitly. The public API does not serialize administrative rows, raw errors or notes. Public incident timelines are separate from check-failure timelines. Maintenance's resource membership expands on save, and public notice copy is separately validated. The page renderer runs server-side through the private internal API URL, then refreshes every thirty seconds; hash-linked tabs can share Overview/Incidents/Maintenance/Metrics/text views. All text is escaped. Public output is no-store to immediately reflect eligibility changes and avoid stale disclosure.

Discord notification jobs capture the validated template at enqueue time, with safe monitor name/reason/duration/timestamp and no target. Route/event selection occurs transactionally with incident events; DOWN cooldowns suppress repeat failures per monitor/channel while RECOVERED remains eligible. Provider retries remain durable. Rendered embeds are length-validated and all mention categories disabled.

Authenticated SSE remains the deliberate choice for one-way invalidations: native EventSource reconnects with the HttpOnly cookie, events contain no monitoring secrets, and the client refetches authorized authoritative data. The stream now revalidates sessions every fifteen seconds and emits a sync invalidation after lag. An expired/revoked session cannot hold a stream indefinitely. Fallback dashboard refresh remains available if streaming is disconnected.
