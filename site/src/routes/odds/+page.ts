import { getJson } from '#lib/data.ts';
import type { OddsRow } from '#lib/types.ts';

export async function load({ fetch }) {
	return { rows: await getJson<OddsRow[]>(fetch, 'odds.json') };
}
