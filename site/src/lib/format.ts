/** Signed rating with one decimal: "+12.3", "−4.0", "0.0". */
export function signed(x: number, digits = 1): string {
	const v = Number(x.toFixed(digits));
	if (v === 0) return (0).toFixed(digits);
	return (v > 0 ? '+' : '−') + Math.abs(v).toFixed(digits);
}

/** Probability as a percentage, with "<1%" / ">99%" at the extremes. */
export function pct(p: number): string {
	if (p <= 0) return '0%';
	if (p >= 1) return '100%';
	if (p < 0.005) return '<1%';
	if (p > 0.995) return '>99%';
	return `${Math.round(p * 100)}%`;
}

/** Spread from the favorite's side: "LSU −5.5". */
export function spread(homeMargin: number, home: string, away: string): string {
	if (Math.abs(homeMargin) < 0.05) return 'Pick';
	const fav = homeMargin > 0 ? home : away;
	return `${fav} −${Math.abs(homeMargin).toFixed(1)}`;
}

export function record(w: number, l: number): string {
	return `${w}–${l}`;
}

export function shortDate(iso: string): string {
	const [y, m, d] = iso.split('-').map(Number);
	const dt = new Date(Date.UTC(y, m - 1, d));
	return dt.toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' });
}
