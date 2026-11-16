# Aegis Monitor

A dark-first, self-hosted monitoring workspace with a Rust control plane, outbound Rust agents, PostgreSQL history, and a SvelteKit dashboard. No Redis, browser-driven monitoring, default accounts, or production demo data.

**Status:** integrated core with the status-platform upgrade and a hardened HTTPS deployment profile; see [known limitations](docs/limitations.md) before production use. This is not an implementation of every optional feature in the original brief.

## Quick start

Requirements: Docker Engine with Compose 2.24.4+, 2+ GB free RAM for normal operation (4+ GB for builds), and a Linux host. Rust builds need a C compiler; development needs stable Rust 1.100+, Node 24, and PostgreSQL 17.

```sh
cp .env.example .env
# Paste independently generated values into .env:
openssl rand -hex 24 # POSTGRES_PASSWORD (URL-safe)
openssl rand -hex 32 # ENCRYPTION_KEY
# APP_URL must exactly match the browser origin, without a trailing slash.
docker compose up -d --build
docker compose ps
```

Open **http://localhost:8080**. The gateway binds loopback by default. If connecting remotely before TLS is installed, use an SSH tunnel (`ssh -L 8080:127.0.0.1:8080 user@server`). Create the initial owner (password ≥12 characters). Public registration then closes. Use Settings to create Admin/Viewer members. A viewer cannot mutate resources or inspect notification credentials.

Only the gateway is published. PostgreSQL, API, and frontend have no public host ports. Migrations run automatically before the API accepts traffic. Existing monitors and queued alerts survive restarts. Do not scale the API service: a PostgreSQL session lock enforces one active scheduler; losing that connection terminates the process for safe restart.

## First monitors

1. **Monitors → Add monitor**. Choose HTTP/HTTPS or TCP. For TCP use `hostname:port` (IPv6: `[address]:port`). HTTP supports GET/HEAD, expected status and optional bounded keyword matching. Redirects are deliberately not followed; monitor the final URL.
2. Use **Test connection**, then save. Checks run every 20 seconds or longer with jitter. Defaults require two consecutive failures to open an incident, and two successes to recover.
3. For private/LAN targets, explicitly set `ALLOW_PRIVATE_TARGETS=true` and restart the API. This grants trusted admins access to the control plane’s network, including loopback/metadata endpoints. Keep false on public or shared deployments; add egress firewall rules for stronger isolation.
4. DNS checks support A, AAAA, CNAME, MX, TXT, NS and SRV with exact expected-value matching. JSON API checks assert a JSON Pointer value; Redis PING, SMTP greeting and PostgreSQL SSL negotiation checks use fixed protocol probes. HTTP/JSON support encrypted bearer/basic credentials and custom headers; never put secrets in URLs or metadata. These protocol checks verify reachability/handshake, not authenticated database queries or email delivery. TLS checks validate the chain/hostname and fail below the configured days remaining. Push monitors produce a one-time secret URL: call `curl -fsS -X POST "$AEGIS_PUSH_URL"` at the expected interval. Do not put the URL in shared logs or source code.
5. See detail pages for actual checks, failures, latency, observed coverage, incidents and annotations. Pausing and maintenance exclude time from availability. Definitions are in [architecture](docs/architecture.md).

## Discord and webhooks

Notifications → Add channel → Discord webhook. Paste an HTTPS `discord.com/api/webhooks/...` URL from your Discord channel settings. It is AES-256-GCM encrypted at rest and never returned. **Send test** queues a real delivery; inspect the history for `sent` or retry/failure. Edit destinations in place, select DOWN/RECOVERED/DEGRADED events and monitor routes, and set a per-monitor failure cooldown. Empty monitor routes mean all monitors. The Discord message designer supports validated template variables, editable fields, colors, footer, timestamp, avatar and thumbnail, plus a server-rendered preview. Saving never sends a message; Send test does. Queued deliveries retain their template snapshot. Recovery is never suppressed by the failure cooldown. Removing the last explicitly routed monitor disables that channel instead of turning it into an all-monitor destination. DEGRADED is reserved; current checks primarily emit UP/DOWN. Provider failures retry eight times with exponential backoff, including after a restart. Generic JSON webhooks are also supported.

Discord mentions are off by default; each channel can allow user, role, or @everyone/@here mentions written in message content. Template variables cannot add pings. Alerts include name, state, safe reason, timestamp and a dashboard link; they intentionally omit target URLs, which may contain secrets. Delivery is **at least once**: a process crash after the provider accepts a request but before the DB commit may repeat a message. State transitions have durable incident/event deduplication.

## Enroll Ubuntu / Hetzner agents

Servers → Connect server → name → save the one-time credentials. The agent needs outbound HTTPS to APP_URL and no inbound port. Use a separate agent for each OS user when PM2 daemons run as different users.

```sh
cargo build --release -p aegis-agent
sudo install target/release/aegis-agent /usr/local/bin/aegis-agent
sudo install -d -m 0750 -o bot -g bot /etc/aegis
sudo install -m 0600 -o bot -g bot deploy/agent.env.example /etc/aegis/agent.env
# Edit /etc/aegis/agent.env: URL, token, PM2 home and absolute pm2 binary.
# Edit service User/Group/ReadWritePaths to match your PM2 owner.
sudo install -m 0644 deploy/aegis-agent.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now aegis-agent
journalctl -u aegis-agent -f
```

- PM2: the agent executes the configured binary with the fixed argument `jlist`, a five-second deadline and a 1 MiB output cap. It never executes monitor-supplied commands. Set `AEGIS_PM2_HOMES=/home/bot/.pm2:/home/bot/.pm2-other`. Homes must be accessible to the service account. An existing-app PM2 systemd template is in `deploy/pm2-bot.service`; review its user/binary paths, and run `pm2 save` as that owner before enabling it. Avoid pointing at an absent PM2 home: PM2 may start a new daemon. The service needs Node/PM2 installed with the correct absolute path and PATH for Node; see [operations](docs/operations.md).
- Docker: opt in with `AEGIS_DOCKER=true`. The agent uses Bollard’s local Docker Engine API for discovery, inspection, HEALTHCHECK and restart counts. Docker socket access is effectively root access; a socket mount with `:ro` does not restrict API methods. Never expose Docker’s API publicly or mount it into the API/frontend. Prefer an operator-maintained restricted local socket proxy permitting only required GET endpoints. See operations for alternatives.
- After connecting, **Monitor** individual discovered containers/apps, or **Monitor host health**. Discovery alone does not create checks or alerts. Agent last-seen is independent of service checks. Missing heartbeats (>60 seconds) fail associated host/service checks at their next scheduled evaluation, subject to thresholds.
- A running container without a Docker HEALTHCHECK is only a liveness signal. Pair it with an HTTP/TCP monitor for application health.
- **A server cannot report its own complete outage.** Run Aegis elsewhere (or use an independent external observer). Add a host monitor for each agent to enable heartbeat alerts.

Optional Compose agent: set `AEGIS_AGENT_TOKEN`, a reachable `AEGIS_URL`, and `ALLOW_INSECURE_HTTP=true` only for isolated local HTTP development, then `docker compose --profile agent up -d --build agent`. Its host metrics describe its container/namespace; use systemd for full host visibility. PM2 and Docker adapters are disabled in this isolated profile.

## Status pages & planned work

**Premade monitors** has live component views for Cloudflare, Proton (including VPN), GitHub, Vercel, Discord, Atlassian Developer, Bitbucket, Trello, DigitalOcean, Twilio, 1Password, and Google Cloud. Choose components to create JSON monitors, or create a public status page with one resource per selected component. The Statuspage providers use documented public v2 summary endpoints and match components by ID. Google Cloud uses its public product catalog and incident history; its statuses mean no active *public* incident was reported for a product and do not reflect project-specific Service Health. No API key is needed for these public feeds. Provider reports can lag or differ from availability at your location. Microsoft 365 requires a tenant-authorized Microsoft Graph integration with `ServiceHealth.Read.All`; Hetzner's status site and feed need a different connector and are linked in the catalog but are not yet component monitor presets.

Make monitors public eligible in advanced settings. In **Status pages → Resources**, create public-safe service names, group them, and link one or more public monitors. Automatic status aggregates underlying checks; manual operational/degraded/partial outage/major outage/maintenance/unknown overrides require a public reason and can expire. Overrides do not rewrite check history or uptime. Unmeasured resources show no measured uptime.

Create a page with a slug, description, logo, accent, theme, support link and announcement. Select/reorder resources and arrange Overview, Incidents, Maintenance, Metrics and custom plain-text tabs. Public URL: `/status/<slug>`; RSS: `/api/v1/public/status/<slug>/feed`. Public content is server-rendered, refreshes every 30 seconds, and never includes targets, credentials, internal notes or raw errors. Public visibility requires explicit eligibility and page selection. Page/resource edits take effect on the next refresh.

**Incidents** supports manual incidents and editing/publishing safe summaries of measured incidents, severity, investigating/identified/monitoring/resolved phases and timestamped editorial updates. Publishing requires an explicitly selected status page. Automatic incidents stay private until published; raw failures remain private. A measured incident cannot be manually resolved while its monitor is unhealthy. A resource override only changes presentation.

**Maintenance** supports create/edit/cancel for monitors/resources and separate public-safe notices on selected pages. Telemetry continues while new incidents/alerts are suppressed. Existing incidents await confirmed recovery. Resource membership is expanded when the window is saved; edit/save again after changing resource membership. Availability excludes maintenance samples.

## Production HTTPS deployment

Set a real DNS hostname pointing to your server and allow inbound TCP 80/443 (UDP 443 optional). In `.env`, set `AEGIS_DOMAIN=status.example.com`, `APP_URL=https://status.example.com`, `APP_ENV=production`, and independently generated database/encryption secrets. Generate `BOOTSTRAP_TOKEN` with `openssl rand -hex 32`; it is required to claim the first production owner.

```sh
docker compose -f compose.yaml -f compose.production.yaml config --quiet
docker compose -f compose.yaml -f compose.production.yaml up -d --build
```

Open your HTTPS domain and enter the bootstrap token when creating the owner. Keep it out of screenshots/logs; remove it from `.env` after setup and recreate the API. No open registration is enabled afterward. The production overlay removes the loopback gateway port and runs Caddy with persistent certificates. Caddy obtains/renews TLS when operator-owned DNS and network requirements are met. The API refuses HTTP APP_URL in production. Sessions use Secure/HttpOnly cookies; SvelteKit supplies nonce-based script CSP. Session list, revoke and rotation are in Settings → Security; password changes revoke all sessions.

For an existing installation, take an off-host DB/key backup, review [migration notes](docs/operations.md#status-platform-upgrade), then use the same Compose files/project/volumes with `up -d --build`. Never use `down -v` during an upgrade. Alternatively use host Caddy with [deploy/Caddyfile](deploy/Caddyfile). Custom public domains need operator proxy/routing configuration; automatic per-page domain provisioning is not implemented.

Keep admin access restricted to trusted users, use off-host encrypted backups, prompt updates and non-root agents. [Security guidance](docs/security.md). This remains a single-workspace, single-active-scheduler deployment; review [limitations](docs/limitations.md) for missing optional features and scale boundaries.

## Local development and verification

```sh
# Start an isolated development DB (do not expose it beyond loopback):
docker run -d --name aegis-dev-db -p 127.0.0.1:5432:5432 \
  -e POSTGRES_USER=aegis -e POSTGRES_PASSWORD=local-development-only \
  -e POSTGRES_DB=aegis postgres:17-alpine
export DATABASE_URL=postgresql://aegis:local-development-only@localhost:5432/aegis
export ENCRYPTION_KEY="$(openssl rand -hex 32)"
export APP_URL=http://localhost:5173
export ALLOW_PRIVATE_TARGETS=true # local controlled targets only
cargo run -p aegis-server
# second terminal
cd apps/web
npm ci
npm run dev
```

The Vite proxy forwards `/api` to Rust on :3001. Native PostgreSQL integration tests use temporary databases, requiring a role with CREATEDB:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
DATABASE_URL=postgresql://aegis:local-development-only@localhost:5432/aegis cargo test --workspace
cd apps/web
npm run lint
npm run check
npm run test
npm run build
npx playwright install chromium
# API + Vite running, fresh development DB, private targets opted in:
npm run e2e
# For a pre-existing owner set TEST_OWNER_EMAIL and TEST_OWNER_PASSWORD.
# E2E creates real persistent test resources; use only a disposable database.
cd ../..
docker compose config --quiet
```

E2E uses actual HTTP/Webhook servers on 19191/19192, waits for real scheduler cycles, causes an outage, verifies incident/notification/recovery, publishes a public page, and tests mobile overflow. Rust integration tests exercise a Discord-shaped mock with provider failure/retry, CSRF, Viewer restrictions, encrypted secrets, durable incident deduplication and agent token rotation/idempotency. See [verification](docs/verification.md) for this build’s executed results.

## Recovery, backups & troubleshooting

```sh
# Local-only recovery; inject temporary environment without placing password in shell history:
read -r -p 'Email: ' RECOVERY_EMAIL
read -r -s -p 'New password: ' RECOVERY_PASSWORD
export RECOVERY_EMAIL RECOVERY_PASSWORD
docker compose run --rm -e RECOVERY_EMAIL -e RECOVERY_PASSWORD api reset-password
unset RECOVERY_EMAIL RECOVERY_PASSWORD
# Backup (encrypt off-host; keep the matching encryption key separately):
deploy/bin/backup /absolute/secure/path/aegis.sql.gz
# Restore only into an empty replacement DB, API stopped; never drops data:
deploy/bin/restore --into-empty-database /absolute/secure/path/aegis.sql.gz
# See docs/operations.md for production Compose-file selection and restore ordering.
```

Changing ENCRYPTION_KEY without re-encrypting data makes saved webhook and monitor credentials unreadable: replace every encrypted credential after a planned rotation or restore the matching key. DB password changes require altering the existing PostgreSQL role as well as .env; POSTGRES_PASSWORD initializes new volumes only. `docker compose down` preserves data; `down -v` deletes it permanently.

- Origin errors: APP_URL must exactly match scheme, hostname and port in the address bar. Restart API/web after edits.
- Empty or pending checks: inspect `docker compose logs api`; verify DNS and egress connectivity, thresholds, and private-network policy. UNKNOWN coverage is never counted healthy.
- Agent offline: verify HTTPS trust, token, outbound firewall and `journalctl`. Rotate tokens in Servers if leaked; update the agent environment and restart its service.
- Notification pending/failed: inspect history; check the provider URL, its permissions and rate limits. GET APIs never reveal saved webhook secrets.
- Container unhealthy: `docker compose ps` and logs; readiness includes DB migrations. Do not delete the persistent volume to “fix” startup.

## Layout and routes

```
crates/core       domain validation, hysteresis, permissions, uptime
crates/checks     bounded HTTP/JSON/TCP/DNS/TLS/protocol checks and target policy
crates/server     Axum API/auth, SQLx, scheduler, incidents, delivery worker
crates/agent      outbound heartbeat, sysinfo, Bollard, bounded PM2 adapter
apps/web         SvelteKit 3 / Svelte 5 + TypeScript, Tailwind 4, Bits UI, Lucide
migrations       versioned PostgreSQL schema
 deploy          Dockerfiles, Nginx/Caddy, agent env/systemd
 docs            architecture, operations, security, API and limitations
```

Dashboard `/`, `/monitors`, `/monitors/:id`, `/servers`, `/incidents`, `/analytics`, `/notifications`, `/status-pages`, `/maintenance`, `/settings`. Bootstrap/login appear automatically when required. Public `/status/:slug`. Versioned API `/api/v1`; liveness `/healthz`, authenticated DB-readiness `/api/v1/system`. API contracts: [docs/api.md](docs/api.md).
