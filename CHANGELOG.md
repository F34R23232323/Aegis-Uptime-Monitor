# Changes

## Status platform and deployment hardening

- Discord embed designer with validated variables, custom fields/colors/footer/images, safe preview, channel editing, event/monitor routes, failure cooldowns, queued template snapshots and failed-delivery retry.
- Additional JSON assertion, Redis PING, SMTP greeting, PostgreSQL SSL negotiation, DNS record and agent CPU/memory/disk monitors. Encrypted HTTP bearer/basic/custom headers are stored separately and redacted.
- Public resources/groups linked to eligible monitors; audited manual statuses with optional expiry. Overrides never rewrite observations or availability.
- Status-page studio with ordered resources and tabs, themes, HTTPS logo/support, announcement, SSR content, editorial incidents, maintenance notices, metrics and RSS.
- Manual incidents, editable severity/phases/publication scopes, separate private notes and timestamped public-safe updates. Measured incidents cannot be manually resolved while unhealthy.
- Session list, rotation/revocation, ongoing SSE session verification, owner audit history, strict production setup token and HTTPS checks, nonce script CSP, bounded DB/public projections, safer IP policy and batched dashboard history.
- Production Caddy Compose overlay, private-service proxy exclusions, guarded backup/restore scripts and CI coverage.
- Additive migrations preserve existing data. Deletion cleans memberships, disables a channel losing its last explicit route rather than broadcasting, cancels pending alerts for deleted monitors and avoids broadening incident publication when its final selected resource is removed.

Upgrade with a database/key backup and the same Compose project/volumes. Read docs/operations.md and docs/limitations.md. Public subscriptions, email/Telegram alerts, TOTP, ICMP, automatic custom domains, dependency suppression and downsampling remain outside this release.
