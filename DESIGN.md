# Design

Tokens live in `site/src/routes/layout.css`; this file names them and the
rules for using them.

## Theme

Light and dark follow `prefers-color-scheme`, overridable with
`data-theme` on `<html>`. Neutrals are tinted (warm in light, cool blue-grey
in dark); no pure black or white.

## Color

Restrained strategy: tinted neutrals plus one accent.

| Token | Role |
|---|---|
| `--bg`, `--surface`, `--surface-2` | Page, panels, hover and expanded rows |
| `--border`, `--grid` | Dividers; chart gridlines |
| `--text`, `--muted`, `--faint` | Primary, secondary and tertiary text |
| `--accent` | Links that act (Team page, focus ring) |
| `--up`, `--down` | Wins, rises; losses, drops. Always with W/L or ▲/▼ |
| `--series-1..3` | Overall, offense, defense in charts |
| `--band-1` | 90% ranges (rating bars, chart bands) |

## Typography

Inter Variable for everything, with tabular numbers (`.num`) wherever
numbers align. Scale: page title 1.5rem bold; section 1rem semibold; body
0.875rem; secondary 0.75rem. Prose capped at about 64ch.

## Layout

One centered column, max width 72rem, 1rem side gutter. Data sits in
bordered tables on `--surface`; avoid cards inside cards. Tables drop
columns by breakpoint instead of scrolling sideways: phones keep rank, team
and rating.

## Components

- **Rank cell:** rank, movement arrow, 90% rank range below in `--faint`.
- **Interval bar:** rating's 90% range on a shared axis, zero as a hairline,
  axis labels in the column header.
- **Expandable row:** whole row toggles; a chevron button carries
  `aria-expanded`. The panel holds the rank chart and the week log.
- **Charts:** SVG drawn at container pixel width so text stays legible;
  hover readout; end-of-line labels instead of legends where possible.

## Motion

150 to 250 ms, ease-out-quart, opacity and transform only. Used for state
(row reveal, chevron rotation), never decoration. Disabled under reduced
motion.
