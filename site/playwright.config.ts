import { defineConfig } from '@playwright/test';

// Browser tests against the built site (run `npm run build` first).
export default defineConfig({
	testDir: 'e2e',
	use: { baseURL: 'http://localhost:4803' },
	webServer: {
		command: 'npx vite preview --port 4803 --strictPort',
		url: 'http://localhost:4803',
		reuseExistingServer: false
	}
});
