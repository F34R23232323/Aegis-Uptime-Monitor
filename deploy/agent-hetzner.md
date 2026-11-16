# Hetzner host agent

`aegis-agent-hetzner.service` is installed and enabled at boot. It runs as `appsvc`, the account that owns the existing production PM2 daemon, using `/usr/local/bin/aegis-agent`. The enrollment token is held in `/etc/aegis/agent-hetzner.env` (`0640`, `root:appsvc`) and is never stored in this repository.

The agent sends its heartbeat to the same-host API at `http://127.0.0.1:5931` with `ALLOW_INSECURE_HTTP=true` for loopback only. The public Cloudflare route rejected agent POSTs, while the loopback API accepted them. This agent reports host health and discovers PM2 applications from `/var/lib/appsvc/.pm2` using `/usr/bin/pm2`. Docker discovery is enabled through `DOCKER_HOST=unix:///run/aegis-docker-proxy/docker.sock`.

After correcting the service account, the `hetzner` server received fresh version `0.1.0` heartbeats with host metrics, eight PM2 applications, and no PM2 discovery errors. After enabling Docker, the server inventory reported 54 containers with no Docker discovery error. The `f34rless` PM2 home is a separate, currently empty daemon. The existing apps were moved to `appsvc` on 2026-11-14, before this agent installation.

`aegis-docker-proxy.service` runs as the dedicated `aegisdproxy` account. That account has Docker-group access, which is host-equivalent privilege. The agent account `appsvc` does not belong to the Docker group; it can only connect to the proxy's `0660` Unix socket. The proxy accepts container list and inspect GETs, returns limited fields, removes labels and environment values, and rejects write requests. The local allowlist test and live request checks passed. Review the proxy code before upgrades, and do not expose its Unix socket to untrusted accounts.

Check with `systemctl status aegis-agent-hetzner` or `journalctl -u aegis-agent-hetzner -f`. Restart with `sudo systemctl restart aegis-agent-hetzner`.
Check the Docker proxy with `systemctl status aegis-docker-proxy` or `journalctl -u aegis-docker-proxy -f`.

Because the agent and control plane run on the same host, this agent cannot report a complete outage of that host. Use an independent observer for that case.
