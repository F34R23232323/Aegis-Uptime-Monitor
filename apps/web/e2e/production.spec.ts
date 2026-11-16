import { test, expect } from '@playwright/test';
test('HTTPS production bootstrap, secure session, CSRF and real push persistence', async ({
	page,
	baseURL
}) => {
	test.skip(
		process.env.TEST_PRODUCTION_SMOKE !== 'true',
		'Run only against a disposable HTTPS production-profile stack'
	);
	expect(baseURL).toMatch(/^https:/);
	const setup = await page.request.get('/api/v1/setup').then((r) => r.json());
	const email = process.env.TEST_OWNER_EMAIL || 'e2e@example.test';
	const password = process.env.TEST_OWNER_PASSWORD || 'e2e-owner-password-unique';
	if (setup.needs_setup) {
		expect(setup.requires_setup_token).toBe(true);
		const wrong = await page.request.post('/api/v1/setup', {
			headers: { Origin: baseURL! },
			data: { email, password, setup_token: 'incorrect-bootstrap-token' }
		});
		expect(wrong.status()).toBe(403);
		const created = await page.request.post('/api/v1/setup', {
			headers: { Origin: baseURL! },
			data: { email, password, setup_token: process.env.TEST_BOOTSTRAP_TOKEN }
		});
		expect(created.ok(), await created.text()).toBeTruthy();
	} else {
		const login = await page.request.post('/api/v1/auth/login', {
			headers: { Origin: baseURL! },
			data: { email, password }
		});
		expect(login.ok()).toBeTruthy();
	}
	const cookies = await page.context().cookies();
	const session = cookies.find((c) => c.httpOnly);
	expect(session?.secure).toBe(true);
	expect(session?.sameSite).toBe('Strict');
	const response = await page.goto('/');
	expect(response?.headers()['content-security-policy']).toMatch(/script-src[^;]*nonce-/);
	expect(response?.headers()['content-security-policy'].match(/script-src[^;]*/)?.[0]).not.toContain(
		'unsafe-inline'
	);
	await expect(page.getByRole('heading', { name: 'Infrastructure, in harmony.' })).toBeVisible();
	const blocked = await page.request.post('/api/v1/resources', {
		headers: { Origin: 'https://untrusted.example.test' },
		data: { name: 'blocked' }
	});
	expect(blocked.status()).toBe(403);
	const create = await page.request.post('/api/v1/monitors', {
		headers: { Origin: baseURL! },
		data: {
			name: 'HTTPS push check',
			kind: 'push',
			target: '',
			interval_seconds: 20,
			timeout_seconds: 5,
			failure_threshold: 1,
			recovery_threshold: 1,
			public: true,
			config: {}
		}
	});
	expect(create.ok(), await create.text()).toBeTruthy();
	const m = await create.json();
	expect(m.push_token).toHaveLength(64);
	expect((await page.request.post('/api/v1/push/' + m.push_token)).ok()).toBeTruthy();
	await expect
		.poll(
			async () =>
				page.request
					.get('/api/v1/monitors/' + m.id)
					.then((r) => r.json())
					.then((m) => m.state),
			{ timeout: 35000 }
		)
		.toBe('UP');
	const history = await page.request.get('/api/v1/monitors/' + m.id + '/checks').then((r) => r.json());
	expect(history.some((c: { state: string }) => c.state === 'UP')).toBeTruthy();
	const redacted = await page.request.get('/api/v1/monitors/' + m.id).then((r) => r.json());
	expect(redacted.push_token_hash).toBeUndefined();
});
