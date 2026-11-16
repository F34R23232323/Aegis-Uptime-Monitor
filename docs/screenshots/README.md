# Aegis interface gallery

These **38 screenshots** show the actual Aegis Svelte interface populated with **synthetic example data**. Server names, incidents, check results, uptime, users, and status information are illustrative. The screenshots are not evidence of a live deployment or measured reliability. No real credentials or user data are included.

Start with the [overview](#overview), then open the sections you want to inspect. Each image links directly to its full resolution PNG. The gallery includes every primary workspace page, the public status page tabs, first-run and sign-in screens, and the main creation, editing, and configuration views.

## Overview

[![Dashboard with monitor states, uptime and incident summary](dashboard.png)](dashboard.png)

<details>
<summary><strong>Monitors and check configuration</strong> — 7 views</summary>

### Monitor list

[![Monitor list with status and recent checks](monitors.png)](monitors.png)

### Create a monitor

[![Create monitor form](monitor-create.png)](monitor-create.png)

### Advanced monitor settings

[![Advanced monitor settings including retention and organization](monitor-advanced.png)](monitor-advanced.png)

### JSON check configuration

[![JSON monitor target and assertion fields](monitor-json.png)](monitor-json.png)

### Monitor detail

[![Monitor detail with response latency and check history](monitor-detail.png)](monitor-detail.png)

### Edit a monitor

[![Edit monitor form](monitor-edit.png)](monitor-edit.png)

### Encrypted credentials and headers

[![Monitor authentication and request header settings](monitor-credentials.png)](monitor-credentials.png)

</details>

<details>
<summary><strong>Premade monitors</strong> — 2 views</summary>

### Provider catalog

[![Premade provider monitor catalog](presets.png)](presets.png)

### Provider components

[![Provider component selection](preset-components.png)](preset-components.png)

</details>

<details>
<summary><strong>Servers and agent enrollment</strong> — 3 views</summary>

### Server fleet

[![Server fleet overview](servers.png)](servers.png)

### Expanded server inventory

[![Server host resources, disks, Docker and PM2 services](server-detail.png)](server-detail.png)

### Connect a server

[![Server agent enrollment form before credentials are generated](server-connect.png)](server-connect.png)

</details>

<details>
<summary><strong>Incidents, updates, and analytics</strong> — 5 views</summary>

### Incident journal

[![Incident journal](incidents.png)](incidents.png)

### Incident timeline

[![Incident timeline and updates](incident-timeline.png)](incident-timeline.png)

### Edit an incident

[![Incident editing form](incident-edit.png)](incident-edit.png)

### Post an incident update

[![Incident update form](incident-update.png)](incident-update.png)

### Analytics

[![Reliability analytics](analytics.png)](analytics.png)

</details>

<details>
<summary><strong>Notification channels</strong> — 3 views</summary>

### Notification routing and delivery history

[![Notification channels and delivery history](notifications.png)](notifications.png)

### Edit a notification channel

[![Notification channel settings](notification-edit.png)](notification-edit.png)

### Discord message designer

[![Discord embed field configuration](notification-designer.png)](notification-designer.png)

</details>

<details>
<summary><strong>Status pages and resource configuration</strong> — 6 views</summary>

### Status page list

[![Status page list](status-pages.png)](status-pages.png)

### Edit a status page

[![Status page identity and appearance settings](status-page-edit.png)](status-page-edit.png)

### Configure public tabs

[![Status page tab settings](status-page-tabs.png)](status-page-tabs.png)

### Configure public graphs

[![Status page graph settings](status-page-graphs.png)](status-page-graphs.png)

### Public resources

[![Status page resources and published states](status-resources.png)](status-resources.png)

### Edit a resource and status override

[![Resource visibility, linked monitors and manual status override](resource-edit.png)](resource-edit.png)

</details>

<details>
<summary><strong>Maintenance and instance settings</strong> — 4 views</summary>

### Maintenance schedule

[![Maintenance schedule](maintenance.png)](maintenance.png)

### Edit a maintenance window

[![Maintenance window settings](maintenance-edit.png)](maintenance-edit.png)

### Public maintenance notice

[![Maintenance publication settings](maintenance-publication.png)](maintenance-publication.png)

### Instance settings

[![Instance settings with user and session controls](settings.png)](settings.png)

</details>

<details>
<summary><strong>Public status page</strong> — 5 views</summary>

### Overview

[![Public status overview](public-overview.png)](public-overview.png)

### Incidents

[![Public incident history](public-incidents.png)](public-incidents.png)

### Maintenance

[![Public maintenance notices](public-maintenance.png)](public-maintenance.png)

### Metrics

[![Public uptime metrics](public-metrics.png)](public-metrics.png)

### Support

[![Public support information](public-support.png)](public-support.png)

</details>

<details>
<summary><strong>Getting started</strong> — 2 views</summary>

### First-run owner setup

[![First-run owner account setup](setup.png)](setup.png)

### Sign in

[![Sign-in screen](sign-in.png)](sign-in.png)

</details>

## Recreate the captures

The capture script runs the real web app against [synthetic fixtures](../../apps/web/scripts/readme-fixtures.mjs). Start the web dev server from `apps/web` with `npm run dev`, then run `node scripts/capture-readme.mjs` from the same directory. It needs Playwright Chromium, or set `PLAYWRIGHT_CHROME_EXECUTABLE` to a local Chrome executable. The script uses local ports `5173` and `3001`.

[Back to the project README](../../README.md)
