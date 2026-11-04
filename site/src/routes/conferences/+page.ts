import { getJson } from '#lib/data.ts';
import type { ConferenceRow } from '#lib/types.ts';

export async function load({ fetch }) {
	return { rows: await getJson<ConferenceRow[]>(fetch, 'conferences.json') };
}
