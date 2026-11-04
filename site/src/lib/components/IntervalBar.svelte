<script lang="ts">
	// A rating's 90% interval on a shared horizontal scale, with the point
	// estimate marked. Zero (an average FBS team) is drawn as a hairline.
	let {
		value,
		sd,
		min,
		max
	}: { value: number; sd: number; min: number; max: number } = $props();
	const z = 1.645;
	const x = (v: number) => `${(100 * (Math.min(max, Math.max(min, v)) - min)) / (max - min)}%`;
</script>

<div class="relative h-4 w-full" aria-hidden="true">
	<div class="absolute top-0 bottom-0 w-px bg-border" style="left: {x(0)}"></div>
	<div
		class="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full"
		style="left: {x(value - z * sd)}; right: calc(100% - {x(value + z * sd)}); background: var(--band-1)"
	></div>
	<div
		class="absolute top-1/2 h-3 w-[3px] -translate-x-1/2 -translate-y-1/2 rounded-full"
		style="left: {x(value)}; background: var(--series-1)"
	></div>
</div>
