# Verification record

## Current changes

Rust 1.100.0 and the locked web dependencies were installed for this session. Configurable push heartbeat grace, bounded network check retries, optional bounded HTTP redirects, and a native API systemd template were added after the historical verification runs below. `cargo check --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the core and checks crate tests (17 passed, including redirect and cross-origin credential tests), Svelte type checking (zero diagnostics), web lint, web unit tests (5 passed), and the production web build passed. The API bind change compiled under Clippy; the native systemd unit was not installed because this is not an operator host. `systemd-analyze verify` could not validate its ExecStart path because the binary has not been installed at `/usr/local/bin/aegis-server`. The remaining items in limitations.md are not implemented or verified by these checks.

An internal bearer-protected Prometheus aggregate exporter, configurable per-monitor raw-check retention, and monitor-list page controls were added subsequently. The exporter token unit test, core retention-validation test, workspace Clippy, Svelte type checking, web unit tests and production web build passed. No live Prometheus scrape or database-backed retention/prune test has been performed; no large-installation load test has been performed.

Verified on Linux x86_64 in the supplied workspace, 2026-11-15. Versions were resolved from registries and checked against package metadata/types, then locked. Rust 1.100.0, Node 24.22.1, PostgreSQL 17, current stable SvelteKit 3.2.0 / Svelte 5.57+, Vite 8.3.8, Vitest 5.1.0.

## Status-platform upgrade verification

Verified 2026-11-15 against real isolated PostgreSQL 17 databases and release Docker images.

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: **36 passed** (11 network/protocol, 3 core, 22 server). Added migration-upgrade preservation, public resource/override expiry and redaction, manual/editorial incidents, page scope/tabs validation, encrypted HTTP auth/header guards, Discord template snapshots/routes/preview, maintenance publication, session rotation, Viewer restrictions, dashboard batching/counts, CPU/memory/disk/load thresholds and deletion membership/routing/publication integrity. Fixed protocol probes are tested against controlled HTTP/TCP servers, including fragmented SMTP responses.
- Web format/lint, strict Svelte/TypeScript checks (zero errors/warnings), **5 unit tests**, and Node-adapter production build: passed.
- Browser suite against Rust+Vite: **2 passed**, production-only test intentionally skipped. Real HTTP outage → incident → durable webhook → recovery → public page (52.0s); resources → light branded page → custom Support tab → incident/editorial update → safe public content → metrics/maintenance sections → Discord embed preview/save → secret redaction (13.3s). Mobile overflow and browser runtime errors checked.
- Release images: API, frontend and optional agent targets built successfully. Docker Compose base and HTTPS production overlay validate. A Docker VFS storage limit interrupted one intermediate frontend export; clearing task-generated build caches and rebuilding resolved it without touching database volumes.
- Release-container browser suite over HTTPS: **2 passed**, exercising the resource/tab/incident/embed workflow and production security/push flow against built images.
- HTTPS production profile: Caddy local-CA TLS on localhost; setup token enforcement, Secure/HttpOnly/SameSite=Strict cookies, script nonce CSP, CSRF rejection, actual push observation and durable check history passed. Local-CA validation does not establish public ACME issuance or external reachability.
- Production backup/restore scripts: generated a restricted gzip backup, restored into an empty replacement database preserving owner/monitor/check rows, and rejected a second restore into the populated database. API restarted against the restored schema and matching key.
- Additive migrations 1, 2 and 3 applied successfully to the restored populated database; the API starts with the bootstrap token removed after owner enrollment.
- Final API/frontend restart: existing authenticated session, monitor definitions, scheduler state and server-rendered public page survived.
- Final isolated agent image: authenticated outbound heartbeat and real CPU/RAM/disks/network snapshot; individual CPU/memory/disk monitors reached UP using actual scheduler observations. Docker/PM2 adapter implementations retain the prior live baseline verification below.
- Hosted private Sites preview updated separately with synthetic examples only. JavaScript syntax checked; no browser QA was run for the Sites artifact because the required managed preview controller was unavailable. It contains no live API connections, credentials or infrastructure captures.

No real Discord credential, operator-owned public DNS, native systemd host or public ACME deployment was supplied. Vendor delivery remains protocol-mock verified. See [limitations](limitations.md) for scale boundaries and unfinished optional integrations.

## Initial v0.1 baseline checks

- Rust formatting: `cargo fmt --all --check` passed.
- Rust lint: `cargo clippy --workspace --all-targets -- -D warnings` passed.
- Rust tests with real isolated PostgreSQL databases: **15 passed** (6 network-check tests, 3 domain tests, 6 server tests). Tests cover real HTTP/TCP, private/mapped-IP policy, redirect refusal, oversized keyword body, DNS failure, hysteresis/coverage, bootstrap, sessions, Origin/CSRF, Viewer restrictions, CRUD, incident/delivery deduplication, maintenance suppression, public projection, agent idempotency/rotation, and a real HTTP Discord-protocol mock with provider failure/retry and mentions disabled.
- Web formatting: `npm run lint` passed.
- Strict Svelte/TypeScript: `npm run check` passed, zero errors/warnings.
- Web tests: **5 passed**, covering same-origin cookie API writes, backend errors, offline failures, unknown uptime and duration presentation.
- Production SvelteKit build passed using the current stable configuration API, Node adapter and package subpath imports.
- Browser E2E: complete owner/login → HTTP monitor → scheduler UP → simulated 503 outage → incident → actual webhook → recovery → public status → monitor detail → mobile overflow scenario. Uses controlled HTTP services, no seeded monitoring observations. Both the initial core run and current stable SvelteKit 3 rerun passed (latest run: 52.7 seconds).
- Live Rust agent: **five real adapter checks passed**, then repeated successfully after separating discovery from heartbeat scheduling: Docker/PM2 discovery and host metrics; individual monitor UP states; Docker HEALTHCHECK failure plus PM2 stop → DOWN/incidents; container restart plus PM2 restart → UP/recovery; terminate agent → missed heartbeat → host incident. These were actual disposable Docker containers and a real isolated PM2 daemon, not static JSON fixtures.
- Compose config validation and final API/web image builds passed. `docker compose up -d --no-build --wait` passed with all four services healthy. Production-stack smoke checks passed: bootstrap/session/Origin, actual internal HTTP and PostgreSQL TCP checks, durable pause/resume boundaries, public redaction, authorized SSE, and API restart preserving sessions/monitors/history plus continued check execution. The production browser also passed login/live SSE, monitor editing, all eight workspace routes, public status, and a no-runtime-errors assertion. The optional agent image built and its non-root Compose profile successfully enrolled, authenticated and reported actual resource metrics with Docker/PM2 adapters isolated.

## Environment issues encountered and resolved

Rust was initially absent; a local stable toolchain was installed. npm needed a workspace-local writable cache. Docker access required the execution environment’s authorized Docker capability. Docker build dependency downloads initially rejected the environment’s intercepting TLS proxy certificate; an optional build-time `build_ca` secret fixed this while preserving TLS verification. Docker’s VFS storage duplicated large build stages and filled the 32 GB workspace; task-created build caches were removed and Rust source staging consolidated. No existing database data was removed. Checks affected by disk exhaustion were rerun after space was recovered. Production-browser checks also caught and fixed auth-rate-limiting of read-only setup requests; authentication POSTs remain rate-limited. The gateway healthcheck explicitly disables environment proxies, and internal agent gateway traffic bypasses environment proxies.

Dependency audit prompted upgrading the initial SvelteKit/test packages. The current dependency tree has **no high/critical advisories**; npm still reports low-severity upstream advisories in the SvelteKit/Bits UI/plugin dependency tree. `npm audit fix` was applied without forcing an incompatible framework downgrade. This is not a claim of a comprehensive independent security audit.

## Reproduce live adapter verification

Use ONLY a disposable development instance and fixtures. These commands deliberately stop/restart the named fixtures and create persistent test monitors/incidents.

```sh
# Requires Docker, PM2, and an already bootstrapped disposable instance.
printf 'setInterval(()=>{},1000);\n' > /tmp/aegis-pm2-fixture.cjs
export TEST_PM2_HOME=/tmp/aegis-test-pm2
export TEST_PM2_BINARY="$(command -v pm2)"
PM2_HOME="$TEST_PM2_HOME" "$TEST_PM2_BINARY" start /tmp/aegis-pm2-fixture.cjs \
  --name aegis-fixture --no-autorestart
docker run -d --name aegis-agent-fixture \
  --health-cmd='test ! -f /tmp/unhealthy' --health-interval=2s --health-retries=1 \
  alpine:3.22 sleep 3600
cargo build -p aegis-agent
# Set TEST_OWNER_EMAIL and TEST_OWNER_PASSWORD privately, never in a committed file.
export TEST_API_URL=http://127.0.0.1:3001
export TEST_APP_URL=http://localhost:5173
python3 tests/agent_smoke.py
# Remove only these disposable fixtures after verification:
docker rm -f aegis-agent-fixture
PM2_HOME="$TEST_PM2_HOME" "$TEST_PM2_BINARY" kill
```

The live script uses an insecure-HTTP opt-in only for this local test. A production agent requires HTTPS by default. It assumes the Docker and PM2 fixtures already exist and leaves its monitoring records for inspection. No test credentials or tokens are production defaults.

## Boundaries of testing

No real Discord webhook credentials were supplied, so the Discord protocol/delivery path was tested against a controlled HTTP mock; generic webhook delivery was tested end-to-end. Real DNS failure was exercised. There is not yet a controlled expired-public-chain TLS fixture or a live Ubuntu systemd installation test. CI workflows are provided; execution in GitHub Actions itself was not available from this workspace. UI chart tooltips use bounded custom SVG, avoiding a heavy chart dependency; browser snapshots verified desktop/mobile presentation but do not substitute for a comprehensive accessibility audit.

After verification, all task-created Compose containers/volumes, the isolated PostgreSQL test database, Docker fixture and PM2 fixture daemon were removed. Built images and source remain. No test owner or monitoring data is installed by default.
