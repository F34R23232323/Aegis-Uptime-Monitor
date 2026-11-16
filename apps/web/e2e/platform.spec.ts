import { test, expect } from '@playwright/test';
test('resource editor, public tabs, incident publishing and Discord embed editing', async ({
	page,
	baseURL
}) => {
	const stamp = Date.now(),
		name = `Resource ${stamp}`,
		title = `Platform ${stamp}`,
		slug = `platform-${stamp}`;
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	await page.goto('/');
	await page.getByLabel('Email address').fill(process.env.TEST_OWNER_EMAIL || 'e2e@example.test');
	await page
		.getByLabel('Password', { exact: true })
		.fill(process.env.TEST_OWNER_PASSWORD || 'e2e-owner-password-unique');
	await page.getByRole('button', { name: 'Sign in', exact: false }).click();
	await expect(page.getByRole('heading', { name: 'Infrastructure, in harmony.' })).toBeVisible();
	await page.goto('/status-pages');
	await page.getByRole('tab', { name: /Resources/ }).click();
	await page.getByRole('button', { name: 'Create resource', exact: true }).click();
	let d = page.getByRole('dialog');
	await d.getByLabel('Resource name').fill(name);
	await d.getByLabel('Group', { exact: true }).fill('Core services');
	await d.getByLabel('Allow this resource on public pages').check();
	await d.getByLabel('Published resource status').selectOption('operational');
	await d
		.getByLabel('Public override reason')
		.fill('Manually managed service; no measured uptime is claimed.');
	await d.getByRole('button', { name: 'Save resource', exact: true }).click();
	await expect(d).not.toBeVisible();
	await page.getByRole('tab', { name: /Status pages/ }).click();
	await page.getByRole('button', { name: 'Create status page', exact: true }).click();
	d = page.getByRole('dialog');
	await d.getByLabel('Page title').fill(title);
	await d.getByLabel('Slug', { exact: true }).fill(slug);
	await d.getByLabel('Theme', { exact: false }).selectOption('light');
	await d.getByLabel(name, { exact: false }).check();
	await d.getByRole('button', { name: 'Add tab', exact: true }).click();
	await d.getByLabel('Tab label', { exact: true }).last().fill('Support');
	await d.getByLabel('Public text', { exact: true }).fill('Example support information ' + stamp);
	await d.getByRole('button', { name: 'Save status page', exact: true }).click();
	await expect(d).not.toBeVisible();
	await page.goto('/incidents');
	await page.getByRole('button', { name: 'Create incident', exact: true }).click();
	d = page.getByRole('dialog');
	await d.getByLabel('Incident title').fill('Investigating intermittent delays');
	await d.getByLabel('Public message', { exact: true }).fill('Investigating <script>unsafe()</script>');
	await d.getByLabel('Private annotation').fill('PRIVATE internal diagnosis');
	await d.getByLabel('Publish incident and its editorial timeline').check();
	await d.getByLabel(title, { exact: true }).check();
	await d.getByLabel(name, { exact: true }).check();
	await d.getByRole('button', { name: 'Save incident', exact: true }).click();
	await expect(d).not.toBeVisible();
	await page.getByRole('button', { name: 'Post update', exact: true }).first().click();
	d = page.getByRole('dialog');
	await d.getByLabel('Phase', { exact: false }).selectOption('identified');
	await d.getByLabel('Update message').fill('Safe public update ' + stamp);
	await d.getByRole('button', { name: 'Post update', exact: true }).click();
	await expect(d).not.toBeVisible();
	const html = await page.request.get('/status/' + slug).then((r) => r.text());
	expect(html).toContain(name);
	expect(html).not.toContain('PRIVATE internal');
	expect(html).not.toContain('<script>unsafe()');
	await page.goto('/status/' + slug);
	await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
	await page
		.getByRole('navigation', { name: 'Public status sections' })
		.getByRole('link', { name: /^Incidents/ })
		.click();
	await expect(page.getByText('Investigating intermittent delays', { exact: true }).last()).toBeVisible();
	await expect(page.getByText('Safe public update ' + stamp, { exact: true }).last()).toBeVisible();
	await page
		.getByRole('navigation', { name: 'Public status sections' })
		.getByRole('link', { name: 'Metrics', exact: true })
		.click();
	await expect(page.getByRole('heading', { name: 'Measured availability' })).toBeVisible();
	await expect(page.getByRole('cell', { name: '—', exact: true })).toHaveCount(3);
	await page
		.getByRole('navigation', { name: 'Public status sections' })
		.getByRole('link', { name: 'Maintenance', exact: true })
		.click();
	await expect(page.locator('body')).toContainText('No published maintenance windows.');
	await page
		.getByRole('navigation', { name: 'Public status sections' })
		.getByRole('link', { name: 'Support', exact: true })
		.click();
	await expect(page.getByText('Example support information ' + stamp, { exact: true })).toBeVisible();
	await page.setViewportSize({ width: 390, height: 844 });
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
	await page.setViewportSize({ width: 1440, height: 1000 });
	await page.goto('/notifications');
	await page.getByRole('button', { name: 'Add channel', exact: true }).click();
	d = page.getByRole('dialog');
	await d.getByLabel('Name', { exact: true }).fill(`Embed ${stamp}`);
	await d.getByLabel('Enabled', { exact: true }).uncheck();
	await d
		.getByLabel('Webhook URL', { exact: true })
		.fill('https://discord.com/api/webhooks/123456789/synthetic-disabled-channel');
	await expect(d.getByLabel('Embed title', { exact: true })).toBeVisible();
	await d.getByLabel('Message content (outside embed)', { exact: true }).fill('Service {{monitor}} is {{status}}');
	await d.getByRole('checkbox', { name: 'Ping mentioned roles' }).check();
	await d.getByLabel('Embed title', { exact: true }).fill('Custom {{status}} alert');
	await d.getByRole('button', { name: 'Preview message', exact: true }).click();
	await expect(d.getByText('Service Example API is DOWN', { exact: true })).toBeVisible();
	await expect(d.getByText('Custom DOWN alert', { exact: true })).toBeVisible();
	await d.getByRole('button', { name: 'Save channel', exact: true }).click();
	await expect(d).not.toBeVisible();
	const channels = await page.request.get('/api/v1/channels').then((r) => r.json());
	const c = channels.find((c: { name: string }) => c.name === `Embed ${stamp}`);
	expect(c.enabled).toBe(false);
	expect(c.template.title).toBe('Custom {{status}} alert');
	expect(c.template.content).toBe('Service {{monitor}} is {{status}}');
	expect(c.template.mention_roles).toBe(true);
	expect(c.template.mention_users).toBe(false);
	expect(c.secret).toBeUndefined();
	await page.request.delete('/api/v1/channels/' + c.id, { headers: { Origin: baseURL! } });
	expect(errors).toEqual([]);
});
