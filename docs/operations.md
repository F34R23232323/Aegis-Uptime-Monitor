# Operations

## Configuration

| Variable | Required/default | Meaning |
| --- | --- | --- |
| DATABASE_URL | required (Compose derives it) | PostgreSQL connection string; URL-encode non-hex passwords |
| ENCRYPTION_KEY | required, 64 hex chars | 32 random bytes for webhook and monitor credential encryption |
| APP_URL | http://localhost:8080 | Exact browser origin; HTTPS enables Secure cookie |
| APP_ENV | development | production requires HTTPS and first-owner token |
| BOOTSTRAP_TOKEN | required before first production owner | 32+ random characters, remove after setup |
| AEGIS_DOMAIN | production overlay | Operator-owned DNS hostname for Caddy |
| AEGIS_INTERNAL_API_URL | http://api:3001 in Compose | Private SvelteKit server-to-API connection |
| APP_NAME | Aegis Monitor | App/login/sidebar text and notifier footer |
| APP_ACCENT | #a18aff | Dashboard accent as a hex color; invalid values use default |
| ALLOW_PRIVATE_TARGETS | false | Explicit admin networking capability for LAN/loopback targets |
| CHECK_WORKERS | 16, clamped 1–128 | Maximum batch concurrency |
| METRICS_TOKEN | unset | 32+ character bearer token enables internal `/metrics` Prometheus gauge endpoint |
| API_BIND | 0.0.0.0:3001 | API listen socket; native reverse-proxy installs should use 127.0.0.1:3001 |
| RUST_LOG | info | Structured tracing filter; avoid HTTP body/credential logging |
| PORT | 8080 | Loopback gateway port |
| AEGIS_URL | required for agent | Central origin, normally HTTPS |
| AEGIS_AGENT_TOKEN | required for agent | One-time enrollment or rotated server credential |
| AEGIS_DOCKER | false | Opt in to local Docker API adapter |
| AEGIS_PM2_HOMES | empty | Colon-delimited operator-authorized PM2 homes |
| AEGIS_PM2_BINARY | pm2 | Prefer absolute pm2 executable path |
| ALLOW_INSECURE_HTTP | false | Explicit local agent HTTP opt-in |

For Node installed through nvm, PM2’s shebang may require a PATH entry for its corresponding Node binary. Put `PATH=/home/bot/.nvm/versions/node/v24.x.x/bin:/usr/local/bin:/usr/bin` in the agent environment with your actual version path, and set AEGIS_PM2_BINARY to its full path. Avoid root or `sudo pm2`: each PM2 home belongs to its process owner. Multiple OS owners should get separate agents/tokens.

Raw check retention defaults to 90 days and can be set per monitor to 7–365 days. Pruning runs hourly; shorter retention removes raw evidence from availability and latency queries. One monitor at 20-second cadence generates 4,320 checks/day; budget storage and test prune/query load before increasing retention. Agent snapshots overwrite the latest inventory and do not have retained host-metric charts. PostgreSQL backup/restore commands are in the README.

## Safe Docker access

An agent with raw docker.sock access can control the host, regardless of a read-only bind mount. The shipped isolated Compose profile does not mount it. For host installation, either deliberately authorize a trusted agent account, or configure a local socket proxy with only list/inspect GET access. Bollard connects using local Docker configuration/DOCKER_HOST; proxy configuration is operator-supplied, not automatically provisioned. Never set an unauthenticated public DOCKER_HOST. Place a proxy on loopback/private Unix socket and restrict its methods/paths; inspect your chosen proxy’s capabilities before relying on it.

## Health and restart

The optional Prometheus endpoint is served by the API at `/metrics`, not routed through the public gateway. Configure a random `METRICS_TOKEN` of at least 32 characters, then scrape the API from a trusted internal network with `Authorization: Bearer <token>`. It exposes aggregate monitor-state and pending/failed delivery counts. Keep the API port private and protect the token as a credential.

`/healthz` is non-sensitive process liveness for internal healthchecks. `/api/v1/system` is session-protected and tests the DB. Compose gates API on DB readiness and gateway on web/API health. A lost scheduler-lock connection exits the API; restart policy reconnects/reacquires it. Queued checks resume at their next durable due time. Delivery leases expire after a crash and retry later. Queries/transactions fail visibly instead of acknowledging unsaved mutations.

To upgrade: back up DB plus key, review migrations/changelog, pull updated source, `docker compose build`, then `docker compose up -d`. Do not run concurrent versions or downgrade a schema without a tested restore. No automatic migration rollback exists. Postgres runs with a persistent named volume; `down -v` destroys it.

## TLS proxy and custom domains

The supplied Caddy example targets the loopback Nginx port. Operators control DNS, firewall, certificate issuance and renewal. Additional public domains can reverse-proxy `/status/<slug>` and `/api/v1/public/status/<slug>` with custom routing, but the product has no automatic custom-domain mapping or certificate UI. Keep admin paths protected on secondary domains. APP_URL remains the trusted admin browser origin.

## Builds behind a corporate TLS proxy

Both Dockerfiles accept optional BuildKit secret `build_ca`, used only during dependency download. With a Compose build override, add `build.secrets: [build_ca]` and a top-level secret with `file: /path/to/your/trusted-ca.crt`. This retains certificate verification and avoids adding environment-specific roots to release images. Normal public-internet builds need no override. Lockfiles are committed for repeatable compatible versions.

`deploy/pm2-bot.service` is an operator-reviewed template for an existing PM2 application set. Set the correct user/home and absolute Node/PM2 paths, run `pm2 save` as the owner, then enable the unit. This template does not create applications or install Node. Native systemd installation was not executed in the container-based verification environment.

For a native API install, build `aegis-server` with `cargo build --release --locked -p aegis-server`, install the binary at `/usr/local/bin/aegis-server`, create a dedicated unprivileged `aegis` account, and install `deploy/aegis-api.service` at `/etc/systemd/system/aegis-api.service`. Copy `deploy/api.env.example` to `/etc/aegis/api.env`, replace every placeholder, set ownership `root:aegis` and mode `0640`, and keep `API_BIND=127.0.0.1:3001` behind a trusted HTTPS reverse proxy. Run `systemctl daemon-reload && systemctl enable --now aegis-api`; inspect `journalctl -u aegis-api` and the reverse-proxied `/healthz`. The API unit runs the scheduler too: only one API process may hold the PostgreSQL advisory lock. Remove `BOOTSTRAP_TOKEN` from the environment file after owner enrollment and restart the unit. Native web, reverse proxy, PostgreSQL, DNS and public certificate setup remain operator-specific; this unit alone is not an end-to-end native deployment.

## Status-platform upgrade

The additive `0002_status_platform.sql` migration adds resource/page settings, incident editorial timelines, Discord templates/routing, encrypted monitor credentials, session identifiers and audit history. It preserves v0.1 checks and incidents. `0003_reference_cleanup.sql` cleans deleted memberships, disables channels losing their last explicit monitor route, cancels pending alerts for a deleted monitor, and unpublishes incidents when deleting their last explicitly selected resource would otherwise widen scope. Existing automatic incidents remain private; publish a safe summary and select pages explicitly. Existing pages retain direct public-monitor selections and gain default tabs. Existing channels retain encrypted URLs and default event routes. API startup runs migrations before accepting requests; run only one version against the DB. Take a database/key backup before upgrading. Rollback requires restoring that backup with the corresponding old binary; migration downgrade is not automated.

Production deployment uses `docker compose -f compose.yaml -f compose.production.yaml ...` consistently. `AEGIS_DOMAIN` must be operator-owned with working DNS. Set `APP_ENV=production`, `APP_URL=https://your-domain`, and a random `BOOTSTRAP_TOKEN` (32+ characters) before initial setup. The API rejects insecure production origins and weak/repeated encryption bytes. Remove the setup token after the owner is created; registration remains closed. Configure `AEGIS_INTERNAL_API_URL=http://api:3001` for SvelteKit public-page SSR (Compose already does). Never point this private server variable at an untrusted API.

The production overlay removes host access to Nginx and adds Caddy on 80/443 with persistent certificate volumes. Do not delete those volumes during routine updates. Local `localhost` certificates use Caddy's local CA; they do not prove real ACME issuance. Actual DNS, firewall, certificate issuance and external reachability must be checked on your host.

## Backup and restore procedures

`deploy/bin/backup /absolute/path/aegis.sql.gz` uses restrictive permissions, an atomic temporary file and a transaction-consistent PostgreSQL dump. It refuses to overwrite an existing backup. Encrypt/copy it off-host along with a separately protected copy of ENCRYPTION_KEY. With production Compose, set `COMPOSE_FILE=compose.yaml:compose.production.yaml` in the command environment; set `COMPOSE_PROJECT_NAME` too if overriding the project. The scripts inherit normal Compose environment selection.

Restore into a new, empty replacement database: start only `db`, keep `api` stopped so migrations do not prepopulate tables, set the matching key and database credentials, then run `deploy/bin/restore --into-empty-database /absolute/path/aegis.sql.gz`. It validates gzip, refuses a nonempty public schema, uses ON_ERROR_STOP and a single transaction, and never drops existing data. Start the remaining stack after restore. Test a restore on an isolated machine before relying on a backup. The scripts do not encrypt or schedule backups for you.

The application DB pool caps acquisition at five seconds, statements at fifteen seconds, locks at five seconds and idle transactions at fifteen seconds. Public projection has a ten-second deadline and four concurrent requests; Nginx additionally limits public polling. Large histories may need PostgreSQL indexes/partitioning or aggregates beyond this release's fixed limits. Monitor disk usage before increasing check counts.

Internal service names are included in web/Caddy NO_PROXY so an inherited corporate outbound proxy cannot intercept private API/gateway traffic. Preserve those names when adding proxy configuration. Build-time CA secrets are separate from runtime TLS trust; do not disable certificate verification in production.

## Optional HTTPS smoke tests

Against a **disposable** production-profile stack using `AEGIS_DOMAIN=localhost` (Caddy local CA), install Playwright Chromium and run from apps/web:

```sh
TEST_BASE_URL=https://localhost TEST_ALLOW_SELF_SIGNED=true \
TEST_PRODUCTION_SMOKE=true TEST_BOOTSTRAP_TOKEN="$BOOTSTRAP_TOKEN" \
npm run e2e -- --grep 'HTTPS production|resource editor'
```

Set TEST_OWNER_EMAIL/TEST_OWNER_PASSWORD for an existing disposable owner. Run browser suites sequentially; they share Playwright output directories. Some IPv4-only Docker hosts need `NODE_OPTIONS=--dns-result-order=ipv4first` for localhost. TEST_ALLOW_SELF_SIGNED affects only the test browser/request client, never application TLS verification. Real deployment uses a publicly trusted certificate and must not enable test TLS exceptions.
