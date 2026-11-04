import { getOptionalJson } from '#lib/data.ts';
import type { Accuracy, PriorSummary } from '#lib/types.ts';

export async function load({ fetch }) {
	return {
		accuracy: await getOptionalJson<Accuracy>(fetch, 'accuracy.json'),
		prior: await getOptionalJson<PriorSummary>(fetch, 'prior.json')
	};
}
