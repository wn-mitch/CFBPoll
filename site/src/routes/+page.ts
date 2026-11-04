import { getJson } from '#lib/data.ts';
import type { RankRow } from '#lib/types.ts';

export async function load({ fetch }) {
	return { rows: await getJson<RankRow[]>(fetch, 'rankings.json') };
}
