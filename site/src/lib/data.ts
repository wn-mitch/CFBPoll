/** Fetches a generated data file; prerendering inlines the result. */
export async function getJson<T>(fetch: typeof globalThis.fetch, path: string): Promise<T> {
	const res = await fetch(`/data/${path}`);
	if (!res.ok) throw new Error(`missing /data/${path} (${res.status}); run \`just build-data\``);
	return (await res.json()) as T;
}

/** Like getJson, but resolves to null when the file was not generated. */
export async function getOptionalJson<T>(fetch: typeof globalThis.fetch, path: string): Promise<T | null> {
	const res = await fetch(`/data/${path}`);
	return res.ok ? ((await res.json()) as T) : null;
}
