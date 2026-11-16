# Native localhost deployment on port 5930

Installed on the operator host on 2026-11-15:

- `aegis-api-5930.service`: release API and scheduler, bound to `127.0.0.1:5931`.
- `aegis-web-5930.service`: SvelteKit production build, bound to `127.0.0.1:5932`.
- `aegis-gateway-5930.service`: Nginx, bound to `127.0.0.1:5930` and `[::1]:5930`.
- PostgreSQL database and login role: `aegis_5930`. This is separate from existing databases.

All three units are enabled at boot with `Restart=always` and a five-second restart delay. The API environment is in `/etc/aegis/api-5930.env` (root-only). The one-time owner token is in `/etc/aegis/bootstrap-5930.token` (root-only). No application owner has been enrolled yet.

The public origin is now `https://status.f34rless.dev` (`APP_URL` in `/etc/aegis/api-5930.env`, `ORIGIN` in a systemd web override, and the workspace `.env`). Route that Cloudflared hostname to `http://localhost:5930` on this host. Open the public HTTPS URL and get the setup token with `sudo cat /etc/aegis/bootstrap-5930.token`. After enrolling the owner, remove `BOOTSTRAP_TOKEN` from `/etc/aegis/api-5930.env`, delete `/etc/aegis/bootstrap-5930.token`, and restart `aegis-api-5930.service`.

Check status with `systemctl status aegis-api-5930 aegis-web-5930 aegis-gateway-5930`. Check logs with `journalctl -u aegis-api-5930 -u aegis-web-5930 -u aegis-gateway-5930 -f`. Restart all three with `sudo systemctl restart aegis-api-5930 aegis-web-5930 aegis-gateway-5930`.

The loopback gateway remains HTTP; Cloudflared provides the public HTTPS connection. The web service reads its build from this workspace, while the API runs the installed binary at `/usr/local/bin/aegis-server-5930`; upgrades must rebuild and reinstall both before restarting. Back up the database and the encryption key together.
