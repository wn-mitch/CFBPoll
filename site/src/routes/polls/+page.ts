import { getOptionalJson } from '#lib/data.ts';
import type { Bias } from '#lib/types.ts';

export async function load({ fetch }) {
	return { bias: await getOptionalJson<Bias>(fetch, 'bias.json') };
}
