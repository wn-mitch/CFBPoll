// Serves the static site, plus /logo/<size>/<id>.png from the
// CollegeFootballData CDN. That CDN sends no CORS headers, so the share image
// draws logos through this same-origin path to keep its canvas exportable.
// vite.config.ts proxies the same path in dev and preview.

interface Env {
	ASSETS: { fetch(request: Request): Promise<Response> };
}

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const m = new URL(request.url).pathname.match(/^\/logo\/(64|128|256)\/(\d+)\.png$/);
		if (!m) return env.ASSETS.fetch(request);
		const upstream = await fetch(`https://cdn.collegefootballdata.com/logos/${m[1]}/${m[2]}.png`);
		if (!upstream.ok) return new Response(null, { status: upstream.status === 404 ? 404 : 502 });
		return new Response(upstream.body, {
			headers: { 'content-type': 'image/png', 'cache-control': 'public, max-age=86400' }
		});
	}
};
