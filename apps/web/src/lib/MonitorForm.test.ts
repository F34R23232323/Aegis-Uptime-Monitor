import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import MonitorForm from './MonitorForm.svelte';

describe('PM2 monitor editor', () => {
	it('renders a selectable discovered application without stray markup text', () => {
		const { body } = render(MonitorForm, {
			props: {
				monitor: { kind: 'pm2', server_id: 'server-1', target: '/var/lib/appsvc/.pm2:1' },
				servers: [
					{
						id: 'server-1',
						name: 'hetzner',
						last_seen: null,
						version: '0.1.0',
						inventory: {
							pm2: [{ id: '/var/lib/appsvc/.pm2:1', name: 'sunstore', healthy: true }]
						}
					}
				],
				onsave: () => {},
				onmessage: () => {}
			}
		});
		const select = body.match(/PM2 application\s*(<select[^>]*>)/)?.[1];
		expect(select).toBeDefined();
		expect(select).not.toContain('disabled');
		expect(body).toContain('value="/var/lib/appsvc/.pm2:1"');
		expect(body).toContain('sunstore');
		expect(body).not.toContain('</label>&gt;');
	});
});
