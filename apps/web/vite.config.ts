import { sveltekit } from '@sveltejs/kit/vite';
import adapter from '@sveltejs/adapter-node';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			adapter: adapter(),
			csp: {
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'https:', 'data:'],
					'connect-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'frame-ancestors': ['none'],
					'form-action': ['self']
				}
			}
		})
	],
	server: { port: 5173, strictPort: true, proxy: { '/api': 'http://127.0.0.1:3001' } },
	test: { include: ['src/**/*.test.ts'] }
});
