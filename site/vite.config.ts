import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: { runes: true },
			adapter: adapter({ pages: 'build', assets: 'build', fallback: '404.html', strict: true }),
			prerender: {
				// Optional data files (accuracy, bias, prior) may be absent; pages
				// render without them. Any other failed fetch fails the build.
				handleHttpError: ({ path, status, message }) => {
					if (status === 404 && /^\/data\/(accuracy|bias|prior)\.json$/.test(path)) return;
					throw new Error(message);
				},
				handleMissingId: 'fail'
			}
		})
	],
	test: {
		expect: { requireAssertions: true },
		include: ['src/**/*.test.ts']
	}
});
