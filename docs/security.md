# Security and trust boundaries

## Operator checklist

- Keep initial setup behind a firewall/tunnel until the Owner is created. Production additionally requires the random bootstrap token; the first valid enrollment owns this single-workspace installation.
- Serve through HTTPS, set APP_URL exactly, permit only 80/443 externally, and keep the gateway bound to loopback behind the supplied reverse proxy. Do not expose API, frontend, PostgreSQL or a Docker daemon directly.
- Set independent high-entropy DB/encryption secrets. Keep .env mode 0600 and out of Git, logs, screenshots and tickets. Back up the encryption key separately from the database. No production default password exists.
- Only invite trusted administrators. Admins can create targets and outgoing webhooks, which intentionally contact network resources. Default private/reserved network blocking reduces accidental SSRF; enabling ALLOW_PRIVATE_TARGETS grants that network capability. Use network-layer egress restrictions around a shared deployment. The deny policy is conservative and is not a complete IANA special-purpose registry.
- Restrict OS permissions on agent tokens/config. Each agent token authorizes only its own server. Rotate compromised tokens and restart that agent. Agents are trusted to describe their own host accurately; central software cannot prove a compromised agent’s observations.
- Docker’s local socket grants powerful daemon access. Never expose it on TCP publicly or mount it into the web/API container. An agent account in the Docker group effectively has root powers; use a local restricted socket proxy and review its permission model.
- Keep backups encrypted off-host, test restores, update patched Rust/JS/container dependencies, review auth and notification histories.

## Implemented controls

Argon2id (library default v0x13, 19 MiB memory, 2 iterations, p=1) with per-password random salt; 256-bit random opaque sessions, SHA-256 verifiers in PostgreSQL, HttpOnly/SameSite=Strict cookies and Secure on configured HTTPS; fresh token at login, 7-day absolute expiry; logout revocation; password-change and CLI recovery revoke all user sessions. Owner bootstrap is protected by a transaction advisory lock and rejects subsequent registration. Password hash/verification work is limited to four concurrent blocking workers. Rate limiting persists in PostgreSQL, supplemented by Nginx per-IP rate limits. Optional per-user SHA-256 TOTP uses a unique encrypted secret, a one-step clock window and stored replay prevention; CLI recovery clears it. Generic invalid login errors and server-side permission checks. Exact Origin checks for authenticated mutations plus login/bootstrap/recovery protect CSRF. Agent and push credentials use separate authentication paths and never accept session privileges.

AES-256-GCM encrypts saved channel and monitor credentials. Public-safe API response construction excludes all monitor targets, config, private descriptions, errors, notes, inventory and secrets. HTTP bearer/basic/custom headers are accepted only through the separate encrypted credentials field. Never place passwords into general monitor metadata.

Request bodies ≤256 KiB; agent inventory ≤200 KB; HTTP keyword body ≤256 KiB; PM2 stdout ≤1 MiB; bounded scheduler workers, deadlines and inventory caps. Parameterized SQL; UUID extractors; server-side role checks. No monitor-supplied shell commands. Check redirects and HTTP environment proxies disabled. Resolved addresses are policy-checked and pinned when connecting. Agent PM2 config is administrator-owned and explicitly allowed, outside API control.

Security headers at Rust/Nginx; same-origin browser API; CORS not enabled. Proxy headers are not used to bypass target or origin validation. API logs omit request paths to protect push URL tokens; reverse proxies operated outside this stack must do the same. SQL errors are exposed as generic failures, with SQLSTATE-only diagnostics to avoid recording sensitive constraint values. Do not turn on verbose transport/body logs in production.

## Boundaries and residual risk

One trusted workspace, not a multitenant service. A Viewer can read private monitoring targets and host inventory but cannot change configuration. Admins manage monitoring and channels; only the Owner manages members. Node serves UI; all security authority and monitoring state belong to Rust/PostgreSQL.

Auth logs record events/user IDs, not device/IP inventory. No emailed reset flow; use gateway rate limiting and protect admin routes from hostile traffic. Public status API runs coverage queries and should receive perimeter caching/rate limiting if heavily exposed. Optional CA BuildKit secrets support corporate proxy builds; never disable TLS verification to make a build pass. Runtime agents require normal HTTPS trust (install your organization CA on the host if necessary).

A complete local-server outage can only be observed independently. This system is not a substitute for a second observer when Aegis shares a failure domain with targets. Alert provider availability is also an external dependency. Delivery is at least once and can repeat after crashes; no guaranteed exactly-once external side effect is claimed.

## Status-platform security additions

Production first-owner enrollment requires a random BOOTSTRAP_TOKEN, verified server-side using a constant-time hash comparison. Never publish that token. Saved HTTP credentials/custom headers use AES-256-GCM and are omitted from GET responses. Reusing saved credentials for a test requires the exact stored target; changing an authenticated monitor's origin requires explicit credential replacement/removal. Reserved/framing headers, URL userinfo and CRLF are rejected. Administrators can intentionally access private networks only after the deployment-wide opt-in; use egress firewall policy for stronger boundaries.

Public projections use allowlisted fields and explicit page/resource membership. Raw monitor error text and private incident annotations do not enter editorial updates. Resource overrides are audited and do not alter measured evidence. Discord templates accept only documented variables, bounded text and HTTPS image URLs; mentions are off by default, can be enabled per channel only for authored message content, and queued jobs preserve a validated template snapshot. Treat template text as public, never as a place for secrets.

SvelteKit renders public content on the server with escaped text and a nonce-based script CSP. SSE emits small invalidations, rechecks the hashed session every fifteen seconds, and closes on revocation, expiration or DB failure. Own-session listing, atomic rotation and revocation are available in Settings. Security/audit lists intentionally omit opaque session verifiers and request credentials. Reverse proxies must not log push token URLs or bodies containing setup/auth secrets.
