# API v1

Base `/api/v1`. JSON success bodies; errors `{ "error": "human-readable summary" }`, with appropriate 400/401/403/404/409/429/500 statuses. UUID paths reject invalid syntax. Bodies max 256 KiB. Cookie auth for admin routes; `Origin` must equal APP_URL on mutations (including bootstrap/login). Same-origin browser fetch credentials. No open CORS. No publicly authenticated controls.

## Authentication

| Method | Path | Body / behavior |
| --- | --- | --- |
| GET | /setup | `{needs_setup,requires_setup_token,name,accent}` (no private instance data) |
| POST | /setup | `{email,password,setup_token?}`; first owner only, sets session cookie |
| POST | /auth/login | `{email,password}`; generic errors, rate-limited, sets session |
| POST | /auth/logout | revoke current token, clear cookie |
| GET | /me | authenticated `{id,email,role}` |
| POST | /auth/password | `{current,password}`; change hash and revoke all sessions |
| POST | /auth/forgot | CLI recovery guidance; no email delivery claim |
| GET/POST | /users | Owner only; POST `{email,password,role:admin\|viewer}` |
| DELETE | /users/:id | Owner only; cannot delete Owner |

## Monitoring

GET routes require any workspace role; writes require Admin/Owner.

`POST /monitors`, `PUT /monitors/:id` accept:

```json
{"name":"API","kind":"http","target":"https://example.com/health","interval_seconds":30,"timeout_seconds":10,"failure_threshold":2,"recovery_threshold":2,"group_name":"production","description":"Health endpoint","tags":["api"],"public":false,"server_id":null,"config":{"method":"GET","status":200,"keyword":"healthy"}}
```

Types: http/json/tcp/dns/tls/redis/smtp/postgres/push/docker/pm2/host/cpu/memory/disk. Timing: interval 20–86400s, timeout 1–30s and below interval, thresholds 1–10. Agent types require an existing server UUID; target is the discovered ID. Kind is immutable on update. Unsupported config keys are rejected. Configuration supports http method/status/keyword, dns record_type/expected, JSON json_pointer/json_expected, TLS warning_days and agent cpu_threshold/memory_threshold/disk_threshold/disk_mount/load_threshold. Push creation returns a one-time `push_token`; later GETs exclude its verifier. Tokens cannot be retrieved.

| Method | Path | Returns / action |
| --- | --- | --- |
| GET | /monitors | latest monitor definitions/state, no push secrets |
| GET/DELETE | /monitors/:id | monitor / delete monitor+history |
| POST | /monitors/:id/toggle | pause/resume, resets evidence counters |
| POST | /monitors/test | MonitorInput, actual bounded network test without persisting |
| GET | /monitors/:id/checks?limit=120 | chronological latest checks; limit clamped 1–1000 |
| GET | /monitors/:id/uptime?days=1 | time-weighted coverage; days clamped 1–90, nullable uptime |
| GET | /incidents | latest 200 with monitor name, timestamps, cause, note |
| PUT | /incidents/:id/note | private `{note}` annotation |
| GET | /events | authorized SSE invalidations; reconnect and refetch |

Uptime: `{days,uptime,covered_seconds,up_seconds,down_seconds}`. A null percentage is unknown, not 100%. Checks have `ok,state,latency_ms,error,observed,created_at`. `state` includes anti-flapping; `ok` is the raw observation. See architecture for coverage/hysteresis semantics.

## Agents and heartbeats

| Method | Path | Behavior |
| --- | --- | --- |
| GET/POST | /servers | latest safe inventory; POST `{name}` → one-time `{id,token,api_url}` |
| POST | /servers/:id/rotate | immediately revoke old token, return new one once |
| DELETE | /servers/:id | revoke/delete; remove referencing monitors first |
| POST | /agent/heartbeat | independent `Authorization: Bearer <agent token>` |
| POST | /push/:token | unique secret token, no admin session required; records last_push |

Agent report `{submission_id:uuid,version:string,inventory:object}`. Server ownership comes only from hashed bearer authentication. `submission_id` deduplicates retries. Inventory fields: `host`, `docker[]`, `pm2[]`, adapter errors. Service IDs select monitors locally; agents cannot directly insert monitor definitions, select another server, or mutate incidents. See agent code for typed snapshot construction. Payload bounds apply before DB writes. Push sends no body and only records success; failure is a missed deadline at interval plus one interval of grace.

## Notifications, maintenance and status

| Method | Path | Body / action |
| --- | --- | --- |
| GET/POST | /channels | Admin/Owner; create `{name,provider:discord\|webhook,secret:url}`; GET redacted |
| DELETE | /channels/:id | delete destination; queued jobs fail safely |
| POST | /channels/:id/test | durable test job, not synchronous “sent” claim |
| GET | /deliveries | Admin/Owner; latest 100, state/attempts/safe failure |
| GET/POST | /maintenance | POST `{reason,starts_at,ends_at,monitor_ids:[uuid]}`; RFC3339 dates |
| DELETE | /maintenance/:id | remove scheduled/current window |
| GET/POST | /status-pages | POST `{slug,title,description,accent,monitor_ids:[uuid]}` |
| PUT/DELETE | /status-pages/:id | same page body / delete |
| GET | /public/status/:slug | unauthenticated public-safe projection only |
| GET | /system | authenticated DB readiness, version and safe settings |

Public projection: page title/description/accent/slug and ordered **public** selected monitors, each with name/state, latest 60 check states/times, incident start/end only, and 24h/7d/30d coverage. Private selected monitors are silently omitted. No generic monitor struct serialization is used here.

## Current protocol limits

This is a single workspace. List APIs do not yet implement cursor pagination/filter parameters; bounded history limits are documented above. The UI’s search/type/status filters apply to fetched monitors. Unknown endpoints return ordinary 404; no comprehensive OpenAPI generator is currently wired. Error correlation IDs, per-route request metrics, bulk APIs and a version-negotiated agent capability handshake are future work.

## Status-platform additions

All write endpoints below require Owner/Admin plus the exact trusted Origin. Viewers can read operational data but cannot edit publication or notification settings. UUIDs, lengths, selected memberships and resource eligibility are validated server-side. Errors are JSON `{ "error": "safe summary" }`.

- `GET /monitors?limit=500&offset=0&q=&state=&kind=&server_id=`: paginated definitions (no encrypted credentials); `GET /monitors/search-count` accepts the same filter parameters and returns the matching total; `GET /monitors/counts`: global effective state counts/open incidents; `GET /monitors/summaries?days=1`: batched latest-100 histories and coverage-aware uptime.
- `GET /servers/:id/metrics?days=1`: authenticated CPU, memory and disk percentage history in bounded 5-minute (1 day), 30-minute (7 days) or 2-hour (30 days) averages. Raw five-minute buckets are pruned after 90 days.
- Monitor POST/PUT accepts `credentials: {bearer?,basic_username?,basic_password?,headers?,clear?}`. Omit to preserve; `clear:true` removes. `POST /monitors/test` accepts `monitor_id` to test unchanged stored credentials. JSON kind config uses `json_pointer` and `json_expected`; inspect MonitorForm and core validation for the canonical fields.
- Monitor POST/PUT accepts optional `depends_on: uuid|null` for one direct alert dependency. Cycles and missing parents are rejected. While a parent monitor is DOWN, a child's queued DOWN delivery waits; its incident and check history continue.
- `GET/POST /resources`, `PUT/DELETE /resources/:id`: `{name,description,group_name,monitor_ids,public,override_state,override_reason,override_until}`. Null override derives from measured state. States: operational/degraded/partial_outage/major_outage/maintenance/unknown.
- `GET/POST /status-pages`, `PUT/DELETE /status-pages/:id`: branding plus `resource_ids`, ordered `tabs:[{id,label,kind,body?}]`, theme/logo/support URL and label/announcement/footer text. Tab kinds overview/incidents/maintenance/metrics/text. Public eligibility is required.
- `GET/POST /incidents`, `PUT /incidents/:id`, `GET/POST /incidents/:id/updates`: manual/editorial lifecycle, severity, phase, published flag, public_message, resource_ids/status_page_ids. `PUT /incidents/:id/note` remains private annotation.
- `GET/POST /maintenance`, `PUT/DELETE /maintenance/:id`: monitor/resource memberships, starts_at/ends_at/reason, public title/message and selected status pages. Resource membership snapshots to affected monitors on save.
- `GET/POST /channels`, `PUT/DELETE /channels/:id`: provider is discord, webhook or email; encrypted webhook URL or single recipient email address, events, monitor_ids (empty=all), cooldown_seconds, optional quiet_start_minute/quiet_end_minute (UTC), delay_seconds (DOWN only), reminder_seconds (0 or 300–86400), template. Blank destination on edit preserves the ciphertext. Email requires SMTP_HOST, SMTP_FROM and optional SMTP_PORT/SMTP_USERNAME/SMTP_PASSWORD on the API; SMTP submission requires STARTTLS. `POST /channels/preview` renders a validated Discord template without sending. `POST /channels/:id/test` queues real delivery and bypasses quiet hours; `POST /deliveries/:id/retry` retries a failed job. GET never returns destinations.
- `GET /auth/sessions`, `DELETE /auth/sessions/:id`, `POST /auth/refresh`: own sessions and atomic rotation. `GET /audit`: owner-only bounded administrative history.
- Public unauthenticated read-only: `GET /public/status/:slug`, `GET /public/status/:slug/feed`. Public data is separately projected, no-store and bounded; a complete projection timeout returns 503 rather than partial misleading health.

Prefix all paths with `/api/v1`. The SvelteKit public HTML route `/status/:slug` performs server-side fetch through AEGIS_INTERNAL_API_URL. `GET /setup` advertises requires_setup_token; initial production `POST /setup` must include setup_token.
