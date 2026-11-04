<script lang="ts">
	import Seo from '#lib/components/Seo.svelte';
	import { signed } from '#lib/format.ts';

	let { data } = $props();
	const names: { [k: string]: string } = {
		kalman: 'This model (fresh start)',
		'kalman-prior': 'This model (roster prior)',
		'kalman-score': 'Scores only, no efficiency',
		'legacy-elo': 'The 2020 version of this poll (Elo)',
		'cfbd-elo': "CFBD's Elo"
	};
	const slices: { [k: string]: string } = { Fbs: 'All FBS vs. FBS', FbsEarly: 'Weeks 1–4', Rated: 'All FBS/FCS' };
	const acc = $derived(data.accuracy);
	const sliceKeys = ['Fbs', 'FbsEarly'];
</script>

<Seo title="Method" description="How the poll works: a Kalman filter on scores and efficiency, a roster-churn prior, and honest uncertainty." />

<article class="prose">
	<h1 class="mt-0 mb-4 text-2xl font-bold tracking-tight">How it works</h1>

	<p>
		Every FBS and FCS team has two numbers: an <strong>offense</strong> rating (points it scores above an average team)
		and a <strong>defense</strong> rating (points it prevents). Their sum is the <strong>overall rating</strong>: the
		margin it would beat an average FBS team by on a neutral field. That is the ranking. Each rating also carries an
		uncertainty, which is where the ± and the rank ranges come from.
	</p>

	<h2>Updating after a game</h2>
	<p>
		Before each game the model predicts both teams' scores. Afterwards it compares the prediction with what happened
		and moves both teams' ratings toward the result. How far it moves depends on how sure it was: a team three games
		into the season moves a lot, a team twelve games in moves little. This is a Kalman filter, the textbook version of
		what Elo approximates with one fixed step size.
	</p>
	<p>
		A game provides four readings: each team's score, and each offense's efficiency (expected points added per play,
		with garbage time removed). Efficiency strips out some luck that scores keep, such as fumble bounces and
		pick-sixes. The fitting step decides how much weight each reading gets. Home field is worth
		{data.meta.hfa} points and is zero at neutral sites. Ratings drift a little each week, so a team can genuinely
		improve or decline.
	</p>

	<h2>Conferences start equal</h2>
	<p>
		Each team's strength is its conference's level plus its own deviation. Every conference starts each season at the
		same level. Only cross-conference games played this season move a conference's level, and with few such games the
		model reports a wide uncertainty rather than assuming anything.
	</p>

	<h2>The season start</h2>
	{#if data.meta.uses_prior && data.prior}
		{@const kept = [
			data.prior.blocks.returning && 'returning production',
			data.prior.blocks.continuity && 'roster continuity by position group',
			data.prior.blocks.portal && 'transfer-portal flow',
			data.prior.blocks.talent && 'the recruiting talent composite'
		].filter(Boolean)}
		{@const dropped = [
			!data.prior.blocks.returning && 'returning production',
			!data.prior.blocks.continuity && 'roster continuity by position group',
			!data.prior.blocks.portal && 'transfer-portal flow',
			!data.prior.blocks.talent && 'the recruiting talent composite'
		].filter(Boolean)}
		<p>
			A team starts each season from last season's rating{kept.length ? `, adjusted for ${kept.join(' and ')}` : ''}.
			The weights come from a regression on {data.prior.train_years[0]}–{data.prior.train_years[data.prior.train_years.length - 1]}
			seasons, not from reputation or polls. {#if dropped.length}We also tested {dropped.join(', ')}; none improved
				accuracy, so they are not used.{/if} Starting uncertainty is wide (about ±{(
				1.645 * Math.hypot(data.prior.sd_offense, data.prior.sd_defense)
			).toFixed(0)} points of overall rating), so results take over within a few weeks.
		</p>
		<p>Error predicting each team's end-of-season rating, in points (leave-one-season-out):</p>
		<table class="num mb-4 w-full max-w-xl text-sm">
			<thead
				><tr class="border-b border-border text-left text-xs text-muted"
					><th class="py-1 font-medium">Inputs</th><th class="text-right font-medium">Offense</th><th
						class="text-right font-medium">Defense</th
					></tr
				></thead
			>
			<tbody>
				{#each data.prior.cv as c (c.name)}
					<tr class="border-b border-border"
						><td class="py-1">{c.name}</td><td class="text-right">{c.rmse_o.toFixed(2)}</td><td class="text-right"
							>{c.rmse_d.toFixed(2)}</td
						></tr
					>
				{/each}
			</tbody>
		</table>
		<p>
			Last season's rating does almost all of the work. Roster details, position by position, add little beyond it.
		</p>
	{:else}
		<p>Every team starts the season from its division's average with wide uncertainty.</p>
	{/if}

	<h2>Odds and rank ranges</h2>
	<p>
		The playoff odds come from simulating the rest of the season thousands of times, drawing each team's true strength
		from the model's uncertainty. The same draws give each team's 90% rank range.
	</p>

	{#if acc}
		<h2>How accurate is it?</h2>
		<p>
			Every forecast below was made before the game, on seasons {acc.years} that the fitting never saw. Log loss
			and Brier score measure win probabilities (lower is better; a coin flip scores 0.693 and 0.25). Margin error is
			the average miss in points; the market column is the closing spread's miss on the same games.
		</p>
		{#each sliceKeys as sk (sk)}
			<h3 class="mt-4 mb-2 text-sm font-semibold">{slices[sk]}</h3>
			<div class="overflow-x-auto">
				<table class="num mb-4 w-full text-sm">
					<thead>
						<tr class="border-b border-border text-left text-xs text-muted">
							<th class="py-1 font-medium">Model</th><th class="text-right font-medium">Games</th><th class="text-right font-medium">Log loss</th><th class="text-right font-medium">Brier</th><th class="text-right font-medium">Picked</th><th class="text-right font-medium">Margin error</th><th class="text-right font-medium">Market error</th>
						</tr>
					</thead>
					<tbody>
						{#each acc.rows.filter((r) => r.slice === sk).sort((a, b) => a.summary.log_loss - b.summary.log_loss) as r (r.model)}
							<tr class="border-b border-border">
								<td class="py-1">{names[r.model] ?? r.model}</td>
								<td class="text-right">{r.summary.games}</td>
								<td class="text-right">{r.summary.log_loss.toFixed(3)}</td>
								<td class="text-right">{r.summary.brier.toFixed(3)}</td>
								<td class="text-right">{(100 * r.summary.accuracy).toFixed(1)}%</td>
								<td class="text-right">{Number.isFinite(r.summary.margin_mae) && r.summary.margin_mae !== null ? r.summary.margin_mae.toFixed(2) : '—'}</td>
								<td class="text-right">{r.summary.market_mae !== null && Number.isFinite(r.summary.market_mae) ? r.summary.market_mae.toFixed(2) : '—'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/each}
	{/if}

	<h2>What it does not do</h2>
	<p>
		It does not know about injuries, quarterback changes or coaching changes until they show up in results. Conference
		tiebreakers in the simulation are simplified (record, then head-to-head, then rating). Data comes from
		CollegeFootballData.com.
	</p>
</article>
