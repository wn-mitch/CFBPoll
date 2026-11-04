import { expect, test } from '@playwright/test';

test('clicking a rankings row toggles its week-by-week panel', async ({ page }) => {
	await page.goto('/');
	const row = page.locator('tbody tr', { has: page.locator('a[href^="/team/"]') }).nth(2);
	const toggle = row.getByRole('button');
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');

	await row.locator('td').first().click();
	await expect(toggle).toHaveAttribute('aria-expanded', 'true');
	const panel = page.locator(`#${await toggle.getAttribute('aria-controls')}`);
	await expect(panel.getByRole('img', { name: /Rank by week/ })).toBeVisible();
	// One log entry per week after preseason, newest first.
	const entries = panel.locator('ol > li');
	expect(await entries.count()).toBeGreaterThan(0);
	await expect(entries.first()).toContainText(/Wk \d+|Postseason/);

	await row.locator('td').first().click();
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');
	await expect(panel).toHaveCount(0);
});

test('the team link in a row navigates instead of expanding', async ({ page }) => {
	await page.goto('/');
	const link = page.locator('tbody a[href^="/team/"]').first();
	const href = await link.getAttribute('href');
	await link.click();
	await expect(page).toHaveURL(href!);
	await expect(page.getByRole('img', { name: /Rank by week/ })).toBeVisible();
});

test('rankings fit a phone screen without sideways scrolling', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/');
	await page.locator('tbody tr', { has: page.locator('a[href^="/team/"]') }).first().locator('td').first().click();
	const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
	expect(overflow).toBeLessThanOrEqual(0);
	await expect(page.locator('thead').getByText('Rating', { exact: true })).toBeInViewport();
});

test('projected scores are never negative', async ({ page, request }) => {
	const rows: { slug: string }[] = await (await request.get('/data/rankings.json')).json();
	for (const { slug } of rows) {
		const team = await (await request.get(`/data/teams/${slug}.json`)).json();
		for (const g of team.schedule) {
			expect(g.points_for, `${slug} vs ${g.opponent}`).toBeGreaterThanOrEqual(0);
			expect(g.points_against, `${slug} vs ${g.opponent}`).toBeGreaterThanOrEqual(0);
		}
	}
	const games = await (await request.get('/data/games.json')).json();
	for (const p of [...games.upcoming, ...games.recent]) {
		expect(Math.min(p.home_points, p.away_points)).toBeGreaterThanOrEqual(0);
	}
});
