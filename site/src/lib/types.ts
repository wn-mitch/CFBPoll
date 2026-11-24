// Mirrors crates/cli/src/build/schema.rs.

export interface Meta {
	year: number;
	as_of: string;
	generated: string;
	games_played: number;
	games_remaining: number;
	uses_prior: boolean;
	draws: number;
	hfa: number;
	qb_new_mean: number;
}

export interface Rated {
	value: number;
	sd: number;
}

export interface Record {
	wins: number;
	losses: number;
	conf_wins: number;
	conf_losses: number;
}

export interface GameRef {
	opponent: string;
	opponent_slug: string | null;
	opponent_logo_id: number | null;
	site: 'home' | 'away' | 'neutral';
	points_for: number;
	points_against: number;
}

export interface RankRow {
	rank: number;
	rank_low: number;
	rank_high: number;
	previous_rank: number | null;
	team: string;
	slug: string;
	logo_id: number | null;
	conference: string | null;
	record: Record;
	overall: Rated;
	offense: Rated;
	defense: Rated;
	offense_rank: number;
	defense_rank: number;
	change: number;
	last_game: GameRef | null;
	schedule_strength: number;
	top25_expected_losses: number;
	history: WeekPoint[];
}

export interface WeekPoint {
	label: string;
	rank: number;
	rank_low: number;
	rank_high: number;
	overall: number;
	change: number | null;
	games: GameRef[];
}

export interface TrajectoryPoint {
	label: string;
	overall: number;
	overall_sd: number;
	offense: number;
	defense: number;
}

export interface ScheduleGame {
	id: number;
	date: string;
	week_label: string;
	opponent: string;
	opponent_slug: string | null;
	opponent_rank: number | null;
	opponent_logo_id: number | null;
	site: 'home' | 'away' | 'neutral';
	conference_game: boolean;
	result: [number, number] | null;
	win_prob: number;
	points_for: number;
	points_against: number;
	market_margin: number | null;
}

export interface TeamOdds {
	expected_wins: number;
	expected_losses: number;
	conf_title: number;
	conf_title_game: number;
	playoff: number;
	bye: number;
	semifinal: number;
	title: number;
}

export interface TeamPage {
	row: RankRow;
	trajectory: TrajectoryPoint[];
	schedule: ScheduleGame[];
	odds: TeamOdds | null;
}

export interface GamePrediction {
	id: number;
	date: string;
	week_label: string;
	home: string;
	home_slug: string | null;
	home_rank: number | null;
	home_logo_id: number | null;
	away: string;
	away_slug: string | null;
	away_rank: number | null;
	away_logo_id: number | null;
	neutral: boolean;
	home_win: number;
	home_points: number;
	away_points: number;
	margin: number;
	margin_sd: number;
	market_margin: number | null;
	market_total: number | null;
	result: [number, number] | null;
}

export interface Games {
	upcoming_label: string | null;
	upcoming: GamePrediction[];
	recent_label: string | null;
	recent: GamePrediction[];
}

export interface ConferenceRow {
	name: string;
	level: Rated;
	member_average: number;
	members: string[];
	member_slugs: string[];
	member_ratings: number[];
	member_logo_ids: (number | null)[];
	nonconference_games: number;
}

export interface OddsRow extends TeamOdds {
	team: string;
	slug: string;
	logo_id: number | null;
	conference: string | null;
	rank: number;
}

export interface GroupStat {
	key: string;
	n: number;
	mean_gap: number;
	median_gap: number;
	above_range: number;
	below_range: number;
}

export interface Comparison {
	year: number;
	poll: string;
	week: number | null;
	team: string;
	conference: string;
	poll_rank: number;
	model_rank: number;
	model_low: number;
	model_high: number;
	preseason_ap: number | null;
}

export interface Bias {
	seasons: number[];
	comparisons: number;
	by_conference: { [poll: string]: GroupStat[] };
	anchoring: [string, [number, GroupStat][]][];
	agreement_by_week: [number, number][];
	snubs_by_conference: { key: string; count: number }[];
	latest: Comparison[];
}

export interface Summary {
	games: number;
	log_loss: number;
	brier: number;
	accuracy: number;
	margin_mae: number;
	total_mae: number;
	market_games: number;
	model_mae_on_market_games: number;
	market_mae: number;
}

export interface Accuracy {
	years: string;
	rows: { model: string; slice: string; summary: Summary }[];
}

export interface PriorSummary {
	train_years: number[];
	blocks: { returning: boolean; continuity: boolean; portal: boolean; talent: boolean };
	talent_kept: boolean;
	sd_offense: number;
	sd_defense: number;
	continuity_effects: { group: string; offense: number; defense: number }[];
	cv: { name: string; rmse_o: number; rmse_d: number }[];
}
