import { defineConfig } from '@playwright/test';
export default defineConfig({
	testDir: './e2e',
	timeout: 180000,
	workers: 1,
	expect: { timeout: 15000 },
	use: {
		baseURL: process.env.TEST_BASE_URL || 'http://localhost:5173',
		ignoreHTTPSErrors: process.env.TEST_ALLOW_SELF_SIGNED === 'true',
		actionTimeout: 15000,
		navigationTimeout: 30000,
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure'
	},
	reporter: 'list'
});
