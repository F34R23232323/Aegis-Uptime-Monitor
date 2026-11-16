// Capture the real Svelte UI with synthetic, local-only API responses.
// Run with the Vite dev server at http://127.0.0.1:5173.
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

const baseURL = 'http://127.0.0.1:5173';
const output = resolve('../../docs/screenshots');
const now = Date.now();
const at = (minutesAgo) => new Date(now - minutesAgo * 60_000).toISOString();

const definitions = [
	['Public website', 'https://example.com', 'http', 'UP', 96, 'Web'],
	['API health', 'https://api.example.com/health', 'http', 'UP', 71, 'Services'],
	['PostgreSQL', 'db.example.com:5432', 'tcp', 'UP', 18, 'Infrastructure'],
	['Background worker', 'worker-01', 'pm2', 'DOWN', null, 'Services']
];
const monitors = definitions.map(([name, target, kind, state, latency, group], index) => ({
	id: `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
	name,
	target,
	kind,
	state,
	enabled: true,
	interval_seconds: 30,
	timeout_seconds: 10,
	failure_threshold: 2,
	recovery_threshold: 2,
	group_name: group,
	description: '',
	tags: [],
	public: false,
	server_id: null,
	config: {},
	last_latency: latency,
	last_error: state === 'DOWN' ? 'Process is offline' : null,
	last_checked_at: at(1)
}));
const summaries = monitors.map((monitor) => {
	const checks = Array.from({ length: 48 }, (_, index) => {
		const down = monitor.state === 'DOWN' && index > 37;
		return {
			id: index + 1,
			ok: !down,
			state: down ? 'DOWN' : 'UP',
			latency_ms: monitor.last_latency || 0,
			error: down ? 'Process is offline' : null,
			observed: {},
			created_at: at(48 - index)
		};
	});
	return {
		id: monitor.id,
		checks,
		uptime: {
			days: 1,
			uptime: monitor.state === 'DOWN' ? 98.8 : 100,
			covered_seconds: 86_400,
			up_seconds: monitor.state === 'DOWN' ? 85_363 : 86_400,
			down_seconds: monitor.state === 'DOWN' ? 1_037 : 0
		}
	};
});
const incidents = [
	{
		id: '00000000-0000-4000-8000-000000000101',
		title: 'Background worker offline',
		severity: 'major',
		phase: 'investigating',
		public_message: '',
		published: false,
		resource_ids: [],
		status_page_ids: [],
		updated_at: at(2),
		monitor_id: monitors[3].id,
		monitor_name: monitors[3].name,
		started_at: at(8),
		ended_at: null,
		cause: 'Process is offline',
		note: ''
	}
];

async function mockAPI(page, setup = false) {
	await page.addInitScript(() => {
		window.EventSource = class {
			constructor() {
				setTimeout(() => this.onopen?.(), 0);
			}
			close() {}
		};
	});
	await page.route('**/api/v1/**', (route) => {
		const path = new URL(route.request().url()).pathname.replace('/api/v1', '');
		if (path === '/events') return route.abort();
		const data =
			path === '/setup'
				? { needs_setup: setup, requires_setup_token: false, name: 'Aegis Monitor', accent: '#a18aff' }
				: path === '/me'
					? { id: '00000000-0000-4000-8000-000000000201', email: 'demo@example.com', role: 'owner' }
					: path === '/monitors'
						? monitors
						: path === '/incidents'
							? incidents
							: path === '/servers' || path === '/status-pages'
								? []
								: path === '/monitors/counts'
									? { total: 4, states: { UP: 3, DOWN: 1 }, open_incidents: 1 }
									: path === '/monitors/summaries'
										? summaries
										: [];
		return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(data) });
	});
}

await mkdir(output, { recursive: true });
const browser = await chromium.launch({
	headless: true,
	...(process.env.PLAYWRIGHT_CHROME_EXECUTABLE
		? { executablePath: process.env.PLAYWRIGHT_CHROME_EXECUTABLE }
		: {})
});
try {
	const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
	await mockAPI(page);
	await page.goto(baseURL, { waitUntil: 'networkidle' });
	await page.getByRole('heading', { name: 'Infrastructure, in harmony.' }).waitFor();
	await page.screenshot({ path: resolve(output, 'dashboard.png'), fullPage: true });

	await page.goto(`${baseURL}/monitors`, { waitUntil: 'networkidle' });
	await page.getByText('Background worker').first().waitFor();
	await page.screenshot({ path: resolve(output, 'monitors.png'), fullPage: true });
	await page.close();

	const setupPage = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
	await mockAPI(setupPage, true);
	await setupPage.goto(baseURL, { waitUntil: 'networkidle' });
	await setupPage.getByRole('heading', { name: 'Your command center awaits.' }).waitFor();
	await setupPage.screenshot({ path: resolve(output, 'setup.png'), fullPage: true });
	await setupPage.close();
} finally {
	await browser.close();
}
