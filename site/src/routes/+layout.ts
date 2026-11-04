import { getJson } from '#lib/data.ts';
import type { Meta } from '#lib/types.ts';

export const prerender = true;
export const trailingSlash = 'never';

export async function load({ fetch }) {
	return { meta: await getJson<Meta>(fetch, 'meta.json') };
}
