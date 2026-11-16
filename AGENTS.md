# Aegis Monitor: instructions for coding agents

This file applies to the whole repository. Read it before editing. Current source code and configuration take precedence when this guide or older documentation has drifted. Update this guide when a change makes it inaccurate.

## Product and scope

Aegis Monitor is a self-hosted, single-workspace monitoring service. Rust owns checks, state transitions, persistence, authentication, notifications, and agent ingestion. PostgreSQL stores durable definitions and history. SvelteKit renders the dashboard and public status pages. Rust agents send outbound authenticated host, Docker, and PM2 inventory. Monitoring and alerts must continue without an open browser tab.

The supported deployment is Docker Compose on Linux. The base gateway binds localhost on port 8080; the production overlay adds Caddy HTTPS. There is no default account or production demo data. Production first-owner enrollment needs a bootstrap token. Cargo workspace metadata declares `AGPL-3.0-only`.

Do not claim a proposed or historical feature is implemented without checking current code and `docs/limitations.md`. This is neither multitenant nor active-active. Real provider delivery, public certificate issuance and native host installs require operator environments to verify.

## First steps for every task

1. Read this file and any nested `AGENTS.md` covering the files you will touch.
2. Check `git status --short --branch` and recent commits if Git exists. Preserve user changes. Do not reset, clean, overwrite or commit unrelated work.
3. Locate owning code and tests with `rg`. Read the relevant part of `README.md` and focused `docs/` files; some documentation contains earlier stages alongside later additions.
4. Make a coherent change at the smallest useful scope. Verify at the affected layer.
5. Review the diff and staged file list before each commit. Keep credentials, generated output and test artifacts out of Git.

If the directory is not yet a Git repository, initialize it only when the task authorizes that or explicitly asks for a commit. If a commit is impossible, report the actual obstacle. Never imply a commit happened when it did not.

## Repository map

| Path | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Rust workspace, dependency versions and reproducible lockfile |
| `crates/core/src/lib.rs` | Monitor input validation, state hysteresis, uptime and permission helpers |
| `crates/checks/src/lib.rs` | Bounded network/protocol checks and target address policy |
| `crates/server/src/main.rs` | Startup, settings, encryption, Axum router and shared app state |
| `crates/server/src/auth.rs` | Bootstrap, login, sessions, roles, Origin/CSRF and recovery |
| `crates/server/src/api.rs` | Monitor/server/agent/push APIs, private queries and metrics |
| `crates/server/src/engine.rs` | Scheduler, observation recording, incidents, alert queue and pruning |
| `crates/server/src/credentials.rs` | Monitor credential validation and use |
| `crates/server/src/notifications.rs` | Channels, Discord templates, preview, delivery and retries |
| `crates/server/src/status.rs` | Resources, public pages, editorial incidents, maintenance and audit |
| `crates/server/src/integration.rs` | PostgreSQL-backed API and state integration tests |
| `crates/agent/src/main.rs` | Outbound heartbeat, host metrics, Docker and PM2 inventory |
| `migrations/` | Versioned SQLx PostgreSQL schema; four migrations currently exist |
| `apps/web/src/lib/App.svelte` | Authenticated workspace shell, navigation and data refresh |
| `apps/web/src/lib/api.ts`, `public.ts` | Browser API contracts and public-page types/helpers |
| `apps/web/src/lib/*.svelte` | Feature panels and shared UI |
| `apps/web/src/routes/` | Authenticated entry, public-page SSR and provider proxy |
| `apps/web/e2e/` | Playwright cross-layer browser flows |
| `deploy/` | Dockerfile, reverse proxies, environment examples, systemd and backup tools |
| `compose.yaml`, `compose.production.yaml` | Base containers and Caddy HTTPS overlay |
| `docs/` | API, architecture, operations, security, verification and limitations |
| `.env.example`, `deploy/*.env.example` | Placeholder configuration, not live secrets |
| `.dockerignore`, `.gitignore` | Build-context and Git exclusions |

`SHA256SUMS` is an existing checksum manifest. It may no longer describe a changed tree. Update it only during an intentional release/checksum workflow after checking its scope.

## Runtime flow

```text
Browser -> Nginx gateway -> SvelteKit UI
                         -> Rust Axum /api/v1 -> PostgreSQL
Rust scheduler -> bounded checks -> PostgreSQL -> incidents/delivery jobs
Delivery worker -> Discord or generic webhook
Rust agent -> outbound authenticated heartbeat -> Rust API -> PostgreSQL
Public SvelteKit SSR -> private API public projection -> public status HTML
```

The browser uses same-origin cookie requests and authorized SSE invalidations, then refetches authoritative data. Public HTML is server-rendered through `AEGIS_INTERNAL_API_URL` and refreshed client-side. The provider-status route is a SvelteKit server endpoint. Keep both servers' network trust boundaries explicit.

## User-facing feature map

### Monitors and observations

`MonitorForm.svelte` is the main editor for monitor definitions and test requests. Rust `MonitorInput` in core is the canonical input validator. The server stores definitions and schedules checks; checks code performs the network protocol work. The API then serves definitions, individual history, batched summaries, counts and coverage-aware availability. Monitor kinds currently include HTTP, JSON API, TCP, DNS, TLS, Redis, SMTP, PostgreSQL, push heartbeat, Docker, PM2, host, CPU, memory and disk. The fixed Redis/SMTP/PostgreSQL probes are reachability or handshake checks, not authenticated application transactions. Docker container running state is not equivalent to an application health check unless the container supplies a HEALTHCHECK. Pair service checks with application-level HTTP/TCP monitoring where appropriate.

Monitor interval, timeout, thresholds, retry, redirects, push grace and raw-check retention have bounded input rules. Find the precise accepted fields and ranges in `crates/core/src/lib.rs`, `crates/checks/src/lib.rs` and the form; do not rely solely on examples. Push secrets are returned once and stored as hashes. A delayed or missing push becomes a failed observation after its configured grace. Pausing creates an explicit state boundary. Maintenance keeps telemetry while suppressing new incidents/alerts. In availability calculations, paused/maintenance/unknown gaps are excluded rather than counted as uptime.

### Servers and outbound agents

The Servers UI creates a server record and reveals enrollment credentials once. The agent sends periodic authenticated reports over HTTPS, with host metrics and optional Docker/PM2 inventory. Discovery does not create monitors automatically; the operator chooses what to monitor. Heartbeat freshness is separate from individual service health. For PM2, the agent service should run as the process owner with explicit PM2 homes and a binary path. Separate OS owners need separate agents/tokens. The isolated Compose agent does not have full host visibility. A server cannot reliably report its own total outage, so an independent observer is needed for that failure domain.

### Alerts and notification channels

Notification channels support Discord webhooks and generic JSON webhooks. Discord templates have validated variables, editable embed fields, previews, event choices, explicit monitor routes and per-monitor DOWN cooldowns. Preview/save does not send a message; Send test queues actual delivery. Empty channel routes mean all monitors, and deleting the last explicitly routed monitor must not silently broaden a channel to all monitors. Recovery is eligible even when the DOWN cooldown is active. Jobs persist with template snapshots and retry after process restarts. A remote provider may receive a duplicate if the process crashes after provider acceptance but before DB acknowledgement.

### Status resources, pages and incidents

The status platform separates private monitor evidence from public presentation. A resource can link explicitly eligible public monitors or be manually managed; a resource without measured coverage has no invented uptime. An override can show operational/degraded/outage/maintenance/unknown with a public reason and optional expiry without changing underlying check history. A page selects resources, branding and ordered tabs (Overview, Incidents, Maintenance, Metrics or escaped custom text). Public HTML is at `/status/<slug>`; safe JSON and RSS are under `/api/v1/public/status/<slug>` and its `/feed` route. Public output is no-store and intentionally bounded.

Automatic measured incidents remain private until explicitly published. Editors can add safe summaries, severity, phase and timeline updates to selected pages. A measured incident cannot be manually marked resolved while the monitor remains unhealthy. Maintenance can select monitors/resources and publish separate safe notices. Resource membership expands to monitor IDs when a maintenance window is saved; saving again is needed if resource membership changes. Editorial/public data must not inherit raw failure strings or private notes.

### Presets and provider data

`presets.ts`, `PresetCatalog.svelte` and `PresetFolders.svelte` define premade external-provider choices; the SvelteKit provider-status route fetches selected public feed data. Provider reports represent the provider's published status, which can lag or differ from local availability. Some catalog entries are links rather than component monitor presets. When changing a provider adapter, verify the current public endpoint, component identifier matching and safe failure behavior. Avoid silently treating a failed provider fetch as operational.

### Account and security settings

The first successful owner enrollment closes public setup. Owner/Admin/Viewer roles have different powers; the Owner manages users. Sessions use opaque tokens with hashed verifiers and HttpOnly cookies; security settings list/revoke/rotate sessions. Password change and CLI recovery revoke sessions. Production bootstrap requires a separate token that should be removed after enrollment. `SecurityPanel.svelte` and `auth.rs` must stay aligned. Login and setup are rate-limited and mutation Origin checks include unauthenticated auth actions.

## Rust behavior that must remain true

- Monitors start `PENDING`. Consecutive observations meet failure and recovery thresholds. `PAUSED`, `MAINTENANCE` and `UNKNOWN` are distinct states; unobserved time is never invented as healthy time.
- Availability is confirmed UP time divided by confirmed UP plus DOWN time, subject to coverage bounds. No measured coverage is `null`, never 100%. Preserve this through database queries, DTOs, types and UI copy.
- One API instance owns the PostgreSQL advisory lock for scheduling. Losing the lock connection terminates the process. Do not run multiple active schedulers.
- Recording an observation, updating counters/state, opening or closing an incident and enqueuing notifications belong in one transaction. Preserve uniqueness, crash recovery, retries and at-least-once external delivery semantics.
- Network probes have bounded deadlines, response sizes and address handling. Private/reserved targets are rejected by default; `ALLOW_PRIVATE_TARGETS` is a deliberate deployment-wide opt-in. Revisit DNS, redirects, credential forwarding and proxy behavior when editing checks.
- HTTP/JSON credentials and channel secrets are encrypted at rest and redacted from GET responses. Never put credentials in targets, metadata, messages or logs.
- Agent bearer tokens are stored as hashes. The token determines server identity, not a submitted ID. Submission UUIDs deduplicate retries. The agent cannot create monitor definitions.
- PM2 uses configured `jlist`, not monitor-supplied shell commands. Docker socket access grants powerful host control even with a read-only mount.
- Admin/Owner can write; Viewer cannot; Owner controls members. Enforce permissions and exact mutation Origin server-side. Public endpoints use explicit safe projections, not generic serialization of private rows.
- Resource overrides affect presentation, not measured evidence. Incident publication and maintenance notices require explicit page selection and safe editorial text. Never expose raw errors, targets, internal notes or inventory publicly.
- Never edit an applied SQL migration to change existing installations. Add a numbered migration and test upgrades against populated data when relevant.

## Web architecture and conventions

The web app uses SvelteKit 3, Svelte 5, TypeScript, Vite 8, Tailwind 4, Bits UI and Lucide. The Node adapter builds the server. `apps/web/vite.config.ts` sets the adapter, CSP and local `/api` proxy to Rust at `127.0.0.1:3001`. `/` and the catch-all route mount the authenticated app; `/status/[slug]` has a server load function; `/provider-status/[id]` proxies selected public provider data.

Feature components include `Auth`, `MonitorForm`, `ServersOverview`, `NotificationsPanel`, `IncidentsPanel`, `MaintenancePanel`, `StatusStudio`, `SecurityPanel`, `PresetCatalog`, `PresetFolders`, `PublicGraphs`, `Chart`, `Bars`, `Status`, `Modal` and `EmbedPreview`. Inspect an existing component before creating a parallel one. Keep TypeScript API types aligned with Rust responses. Preserve loading, unknown and error states. User text must remain escaped; custom status tabs are plain text, not arbitrary HTML.

The authenticated app handles `/monitors`, `/monitors/:id`, `/servers`, `/incidents`, `/analytics`, `/notifications`, `/status-pages`, `/maintenance` and `/settings` inside `App.svelte`; these paths do not each have a separate Svelte route file. Public pages use `/status/:slug`.

## Configuration and deployment

Read `docs/operations.md` and `.env.example` for the full variable contract.

| Area | Variables | Invariant |
| --- | --- | --- |
| Database/crypto | `POSTGRES_PASSWORD`, `DATABASE_URL`, `ENCRYPTION_KEY` | Generate independent strong values; preserve the key with database backups |
| Origin/setup | `APP_URL`, `APP_ENV`, `BOOTSTRAP_TOKEN`, `AEGIS_DOMAIN` | Exact origin; production requires HTTPS and first-owner token |
| API/runtime | `API_BIND`, `CHECK_WORKERS`, `ALLOW_PRIVATE_TARGETS`, `METRICS_TOKEN`, `RUST_LOG` | Keep API private; private targets are opt-in; metrics token is secret |
| Web/brand | `AEGIS_INTERNAL_API_URL`, `APP_NAME`, `APP_ACCENT`, `PORT` | Internal API is server-only; gateway is loopback by default |
| Agent | `AEGIS_URL`, `AEGIS_AGENT_TOKEN`, `ALLOW_INSECURE_HTTP`, `AEGIS_DOCKER`, `AEGIS_PM2_HOMES`, `AEGIS_PM2_BINARY` | Outbound HTTPS by default; host adapters are operator-controlled |

Base Compose starts PostgreSQL, Rust API, SvelteKit web and Nginx. `--profile agent` starts an optional container agent whose metrics describe its container/namespace; install a systemd agent for full host visibility. The production overlay removes the gateway host port and adds Caddy. Do not expose PostgreSQL, API, web or Docker daemon directly. Never use `docker compose down -v` for an upgrade. `deploy/bin/backup` and `deploy/bin/restore` cover backup and restore; restore requires an empty replacement DB and stopped API.

`.gitignore` excludes local credentials, private keys, builds, test output, backups and logs. Keep `.env.example`, `deploy/*.env.example`, `Cargo.lock` and `apps/web/package-lock.json` tracked. Before publishing, review the complete candidate file list and scan for secrets. Ignore rules do not remove a secret already committed; leaked secrets require rotation and deliberate history remediation.

## Working and verification commands

Run Rust/Compose commands at the root; npm scripts in `apps/web`. Use locked installs/builds where supported.

```sh
# Root
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
docker compose config --quiet
docker compose -f compose.yaml -f compose.production.yaml config --quiet

# apps/web
npm ci
npm run lint
npm run check
npm run test
npm run build
```

Server integration tests need a **disposable** PostgreSQL instance and a `DATABASE_URL` role allowed to create temporary databases. Playwright needs a running API and Vite app, controlled test endpoints and a fresh disposable DB. It creates persistent owners, monitors, incidents, channels and pages. Never run E2E against production or an existing user DB. Chromium may need `npx playwright install chromium`. `TEST_OWNER_EMAIL` and `TEST_OWNER_PASSWORD` are only for an existing disposable owner. Check `apps/web/playwright.config.ts` and `docs/verification.md`.

| Change | Useful verification |
| --- | --- |
| Markdown/ignore/examples | Inspect diff, paths/links and representative `git check-ignore` results |
| Core/checks Rust | Format, focused crate tests, workspace Clippy when available |
| API/auth/scheduler/DB | Format, relevant tests with disposable PostgreSQL, Clippy; populated upgrade test for migrations |
| Svelte/TypeScript | `npm run check`, targeted Vitest, `npm run lint`; production build for routes/config |
| Cross-layer flow | Focused Playwright on a disposable instance after API/web checks |
| Compose/proxy/systemd | Compose config validation and focused review; live smoke only on disposable/authorized host |

Report which checks actually ran, which could not run and why. Historical results in `docs/verification.md` do not certify new changes. Do not relax tests just to hide a failure.

## Documentation ownership

- `README.md`: operator setup, initial workflows, agent enrollment and high-level routes.
- `docs/architecture.md`: scheduling, states, coverage, agents, delivery, SSE and publication semantics.
- `docs/api.md`: route contract; compare with current router/handlers before adding endpoints. Earlier sections coexist with later additions.
- `docs/security.md`: operator duties, controls and trust boundaries.
- `docs/operations.md`: environment, deployment, upgrades, backup and troubleshooting.
- `docs/verification.md`: dated executed checks and their limits.
- `docs/limitations.md`: missing and bounded functionality.
- `CHANGELOG.md`: notable user-facing changes, not a replacement for Git history.

Update the relevant documentation when behavior or configuration changes. Write verification entries only for checks actually executed in the stated environment. Do not copy old results forward as new evidence.

## Git and commit policy

**Commit every completed change to Git with a proper, descriptive message.** This includes docs and configuration. For multiple independent changes, make focused commits at coherent checkpoints. Finish work before committing; do not commit broken or incomplete work simply to satisfy the rule.

Before every commit:

1. Inspect `git status --short`, the unstaged diff and any pre-existing user edits.
2. Run appropriate verification and record any unavailable check for the final report.
3. Stage explicit paths for this completed change, rather than `git add -A` in a shared dirty checkout.
4. Inspect `git diff --cached --check`, `git diff --cached --stat` and `git diff --cached`. Ensure no `.env`, token, key, database, build artifact or unrelated file is staged.
5. Use an imperative, concise subject describing action and outcome, preferably `area: action and outcome` (for example `checks: reject unsafe redirect targets` or `docs: document agent deployment and Git workflow`). Add a body with motivation, behavior, migration or verification for nontrivial work.
6. Confirm the commit exists and the worktree contains only expected remaining changes. Tell the user the commit hash and verification summary.

Never amend, force-push, reset or rewrite shared history unless explicitly requested. Do not create a GitHub remote without the user's repository destination. If Git identity is unset, ask for the desired identity instead of inventing one. If Git is unavailable or commits are blocked, give the exact reason and leave a reviewable diff.

**GitHub push permission is conditional: an AI may push commits only when it is 100% sure the push is okay.** Before pushing, confirm the destination repository and branch are the intended ones, inspect the exact commits and file changes being sent, verify that no secrets or unrelated user work are included, run the relevant checks, and confirm the remote branch can be advanced without overwriting others' commits. Use an ordinary push only; never force-push under this permission. If any fact is unknown, a check fails, the remote has unexpected commits, or there is any doubt about safety or destination, stop before pushing and ask the user. A successful local commit alone is not permission to push.

## Security review triggers

Recheck the trust boundary when editing auth, cookies, Origin validation, public projections, agent ingestion, monitor targets, webhook delivery, Docker/PM2 access, encryption, credential rotation, logs, backups or proxy configuration. URLs can contain secrets. Do not read or output live `.env` values for routine work. Do not log push URLs, tokens, provider secrets, encrypted payloads or full request bodies. Use synthetic examples and disposable fixtures.

A routine code request does not authorize modifying a live database, replacing production services, deleting volumes, revoking credentials or sending real notifications. Use disposable verification environments.

## Definition of done

The owning code and docs agree; meaningful checks pass or their limits are reported; the diff has no unrelated, sensitive or generated files; and a descriptive Git commit records the completed work when the checkout is under Git. Tell the user what changed, how it was verified and any material limitation.
