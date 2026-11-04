// Captures the /og route of the built site as static/og.png (1200×630).
import { spawn } from 'node:child_process';
import { chromium } from '@playwright/test';

const port = 4801;
const server = spawn('npx', ['vite', 'preview', '--port', String(port), '--strictPort'], { stdio: 'ignore' });
try {
	const url = `http://localhost:${port}/og`;
	for (let i = 0; ; i++) {
		try {
			if ((await fetch(url)).ok) break;
		} catch {}
		if (i > 50) throw new Error('preview server did not start');
		await new Promise((r) => setTimeout(r, 200));
	}
	const browser = await chromium.launch();
	const page = await browser.newPage({ viewport: { width: 1200, height: 630 }, colorScheme: 'light' });
	await page.goto(url);
	await page.screenshot({ path: 'static/og.png' });
	await browser.close();
	console.log('wrote static/og.png');
} finally {
	server.kill();
}
