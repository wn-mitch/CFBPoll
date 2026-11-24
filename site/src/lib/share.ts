// Draws a ranking's top 25 as a 1080×1350 PNG and hands it to the system
// share sheet, or downloads it where file sharing is unsupported. Logos load
// through the same-origin /logo proxy (worker.ts): drawing them straight from
// their CDN, which sends no CORS headers, would taint the canvas and block the
// export. A logo that fails to load is left blank.
import { record, signed } from './format.ts';
import { site } from './site.ts';
import type { Meta, Rated, RankRow } from './types.ts';

export interface ShareEntry {
	rank: number;
	move: number;
	row: RankRow;
	rating: Rated;
}

// Light theme tokens from layout.css, as sRGB so every canvas parses them.
const c = {
	bg: '#fbfaf8',
	stripe: '#f2f2ef',
	border: '#dfdeda',
	text: '#13161b',
	muted: '#595e64',
	faint: '#82878c',
	up: '#007e46',
	down: '#c13d34',
	series: '#2a78d6',
	band: 'rgba(42, 120, 214, 0.18)'
};
const font = "'Inter Variable', ui-sans-serif, system-ui, sans-serif";
const z = 1.645;
const W = 1080;
const H = 1350;

function loadLogo(id: number | null): Promise<HTMLImageElement | null> {
	if (id == null) return Promise.resolve(null);
	const img = new Image();
	img.src = `/logo/128/${id}.png`;
	return img.decode().then(
		() => img,
		() => null
	);
}

export async function renderTop25(entries: ShareEntry[], title: string, meta: Meta): Promise<Blob> {
	const rows = entries.slice(0, 25);
	const [logos] = await Promise.all([
		Promise.all(rows.map((e) => loadLogo(e.row.logo_id))),
		...['400', '600', '700'].map((w) => document.fonts.load(`${w} 24px 'Inter Variable'`))
	]);
	const canvas = document.createElement('canvas');
	canvas.width = W;
	canvas.height = H;
	const g = canvas.getContext('2d')!;
	g.fillStyle = c.bg;
	g.fillRect(0, 0, W, H);
	g.textBaseline = 'middle';

	const pad = 64;
	const text = (
		s: string,
		x: number,
		y: number,
		size: number,
		weight: number,
		color: string,
		align: CanvasTextAlign = 'left',
		maxWidth?: number
	) => {
		g.font = `${weight} ${size}px ${font}`;
		g.fillStyle = color;
		g.textAlign = align;
		g.fillText(s, x, y, maxWidth);
	};

	text('CFB Poll', pad, 84, 52, 700, c.text);
	text(`${meta.year} · through ${meta.as_of}`, W - pad, 88, 26, 400, c.muted, 'right');
	text(title, pad, 140, 28, 600, c.muted);

	const top = 196;
	const rowH = 42;
	const ratings = rows.flatMap((e) => [e.rating.value - z * e.rating.sd, e.rating.value + z * e.rating.sd]);
	const min = Math.floor(Math.min(0, ...ratings) / 10) * 10;
	const max = Math.ceil(Math.max(...ratings) / 10) * 10;
	const barL = 760;
	const barR = W - pad;
	const x = (v: number) => barL + ((barR - barL) * (Math.min(max, Math.max(min, v)) - min)) / (max - min);

	const teamX = pad + 150;
	text('Team', teamX, top - 18, 18, 600, c.faint);
	text('Record', 560, top - 18, 18, 600, c.faint, 'right');
	text('Rating', 700, top - 18, 18, 600, c.faint, 'right');
	text('90% range', barL, top - 18, 18, 600, c.faint);

	rows.forEach((e, i) => {
		const y = top + i * rowH + rowH / 2;
		if (i % 2 === 1) {
			g.fillStyle = c.stripe;
			g.fillRect(pad - 16, y - rowH / 2, W - 2 * pad + 32, rowH);
		}
		text(String(e.rank), pad + 40, y, 26, 700, c.text, 'right');
		if (e.move !== 0) text(`${e.move > 0 ? '▲' : '▼'}${Math.abs(e.move)}`, pad + 52, y + 1, 16, 600, e.move > 0 ? c.up : c.down);
		const logo = logos[i];
		if (logo) {
			// Fit inside a 32px box, keeping the logo's aspect ratio.
			const s = 32 / Math.max(logo.naturalWidth, logo.naturalHeight);
			const w = logo.naturalWidth * s;
			const h = logo.naturalHeight * s;
			g.drawImage(logo, pad + 104 + (32 - w) / 2, y - h / 2, w, h);
		}
		text(e.row.team, teamX, y, 26, 600, c.text, 'left', 560 - 90 - teamX);
		const r = e.row.record;
		text(record(r.wins, r.losses), 560, y, 24, 400, c.muted, 'right');
		text(signed(e.rating.value), 700, y, 26, 600, c.text, 'right');

		g.fillStyle = c.border;
		g.fillRect(x(0), y - 14, 1.5, 28);
		g.fillStyle = c.band;
		g.beginPath();
		g.roundRect(x(e.rating.value - z * e.rating.sd), y - 6, x(e.rating.value + z * e.rating.sd) - x(e.rating.value - z * e.rating.sd), 12, 6);
		g.fill();
		g.fillStyle = c.series;
		g.beginPath();
		g.roundRect(x(e.rating.value) - 2.5, y - 12, 5, 24, 2.5);
		g.fill();
	});

	const foot = top + 25 * rowH + 44;
	g.fillStyle = c.border;
	g.fillRect(pad, foot - 24, W - 2 * pad, 1.5);
	text('Rating = expected margin vs. an average FBS team', pad, foot + 8, 22, 400, c.muted);
	text(site.url.replace(/^https?:\/\//, ''), W - pad, foot + 8, 22, 600, c.text, 'right');

	return new Promise((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('PNG export failed'))), 'image/png'));
}

export async function shareImage(blob: Blob, name: string, title: string): Promise<void> {
	const file = new File([blob], name, { type: 'image/png' });
	if (navigator.canShare?.({ files: [file] })) {
		try {
			await navigator.share({ files: [file], title, text: site.url });
		} catch (e) {
			if ((e as DOMException).name !== 'AbortError') throw e;
		}
		return;
	}
	const a = document.createElement('a');
	a.href = URL.createObjectURL(blob);
	a.download = name;
	a.click();
	setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
