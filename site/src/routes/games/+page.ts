import { getJson } from '#lib/data.ts';
import type { Games } from '#lib/types.ts';

export async function load({ fetch }) {
	return { games: await getJson<Games>(fetch, 'games.json') };
}
