import { getJson } from '#lib/data.ts';
import type { TeamPage } from '#lib/types.ts';

export async function load({ fetch, params }) {
	return { team: await getJson<TeamPage>(fetch, `teams/${params.slug}.json`) };
}
