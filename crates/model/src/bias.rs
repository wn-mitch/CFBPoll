//! Human-poll bias audit: compares poll ranks with this model's ranks using
//! only the games played before each poll was released.

use crate::data::Week;
use crate::rng::Rng;
use crate::season::Snapshot;
use serde::{Deserialize, Serialize};

/// The snapshot a poll released before `poll_week`'s games should be
/// compared with: the latest one strictly before that week. A postseason
/// (final) poll compares with the last snapshot.
pub fn snapshot_for_poll(snapshots: &[Snapshot], poll_week: Week) -> &Snapshot {
    if poll_week.postseason {
        return snapshots.last().expect("a run has a preseason snapshot");
    }
    snapshots
        .iter()
        .rev()
        .find(|s| s.week.is_none_or(|w| w < poll_week))
        .unwrap_or(&snapshots[0])
}

/// Model rank (1-based) of each of `teams` by overall mean, and its 5–95%
/// rank range from independent normal draws of each team's overall rating.
/// Independence ignores rating correlations; the ranges are approximate.
pub fn ranks_with_range(
    snapshot: &Snapshot,
    teams: &[usize],
    draws: usize,
    seed: u64,
) -> Vec<(usize, usize, usize)> {
    let mean: Vec<f64> = teams.iter().map(|&t| snapshot.ratings[t].overall).collect();
    let sd: Vec<f64> = teams
        .iter()
        .map(|&t| snapshot.ratings[t].overall_sd)
        .collect();
    let rank_by = |vals: &[f64]| -> Vec<usize> {
        let mut idx: Vec<usize> = (0..vals.len()).collect();
        idx.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]));
        let mut r = vec![0; vals.len()];
        for (pos, &i) in idx.iter().enumerate() {
            r[i] = pos + 1;
        }
        r
    };
    let point = rank_by(&mean);
    let mut samples: Vec<Vec<usize>> = vec![Vec::with_capacity(draws); teams.len()];
    let mut rng = Rng::new(seed);
    let mut v = vec![0.0; teams.len()];
    for _ in 0..draws {
        for i in 0..teams.len() {
            v[i] = mean[i] + sd[i] * rng.normal();
        }
        for (i, r) in rank_by(&v).into_iter().enumerate() {
            samples[i].push(r);
        }
    }
    samples
        .into_iter()
        .enumerate()
        .map(|(i, mut s)| {
            if s.is_empty() {
                return (point[i], point[i], point[i]);
            }
            s.sort_unstable();
            let q = |p: f64| s[((s.len() - 1) as f64 * p).round() as usize];
            (point[i], q(0.05), q(0.95))
        })
        .collect()
}

/// One poll-ranked team in one poll release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub year: i32,
    pub poll: String,
    /// Regular-season poll week, or `None` for the final poll.
    pub week: Option<i32>,
    pub team: String,
    pub conference: String,
    pub poll_rank: u32,
    pub model_rank: usize,
    pub model_low: usize,
    pub model_high: usize,
    /// This team's poll rank in the season's preseason AP poll, if ranked.
    pub preseason_ap: Option<u32>,
}

impl Comparison {
    /// Positive when the poll ranks the team better than the model does.
    pub fn gap(&self) -> f64 {
        self.model_rank as f64 - self.poll_rank as f64
    }

    /// The poll rank lies better than the model's 90% range.
    pub fn poll_above_range(&self) -> bool {
        (self.poll_rank as usize) < self.model_low
    }

    pub fn poll_below_range(&self) -> bool {
        (self.poll_rank as usize) > self.model_high
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupStat {
    pub key: String,
    pub n: usize,
    pub mean_gap: f64,
    pub median_gap: f64,
    /// Share of entries where the poll ranks the team better than the
    /// model's 90% range.
    pub above_range: f64,
    pub below_range: f64,
}

pub fn group_stat(key: &str, rows: &[&Comparison]) -> GroupStat {
    let n = rows.len();
    if n == 0 {
        return GroupStat {
            key: key.into(),
            ..Default::default()
        };
    }
    let mut gaps: Vec<f64> = rows.iter().map(|r| r.gap()).collect();
    gaps.sort_by(f64::total_cmp);
    GroupStat {
        key: key.into(),
        n,
        mean_gap: gaps.iter().sum::<f64>() / n as f64,
        median_gap: gaps[n / 2],
        above_range: rows.iter().filter(|r| r.poll_above_range()).count() as f64 / n as f64,
        below_range: rows.iter().filter(|r| r.poll_below_range()).count() as f64 / n as f64,
    }
}

/// Preseason-AP bucket label used for the anchoring analysis.
pub fn preseason_bucket(rank: Option<u32>) -> &'static str {
    match rank {
        Some(1..=5) => "preseason 1-5",
        Some(6..=10) => "preseason 6-10",
        Some(11..=25) => "preseason 11-25",
        _ => "unranked preseason",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::season::TeamRating;

    fn snap(week: Option<i32>, vals: &[f64]) -> Snapshot {
        Snapshot {
            week: week.map(|n| Week {
                postseason: false,
                number: n,
            }),
            ratings: vals
                .iter()
                .map(|&v| TeamRating {
                    offense: v / 2.0,
                    offense_sd: 1.0,
                    defense: v / 2.0,
                    defense_sd: 1.0,
                    overall: v,
                    overall_sd: 1.0,
                })
                .collect(),
        }
    }

    #[test]
    fn poll_uses_only_earlier_games() {
        let snaps = vec![
            snap(None, &[0.0]),
            snap(Some(1), &[1.0]),
            snap(Some(2), &[2.0]),
            snap(Some(4), &[4.0]),
        ];
        let w = |n| Week {
            postseason: false,
            number: n,
        };
        // The preseason poll (week 1) sees no games.
        assert!(snapshot_for_poll(&snaps, w(1)).week.is_none());
        // Week 3's poll sees weeks 1 and 2 only.
        assert_eq!(snapshot_for_poll(&snaps, w(3)).week, Some(w(2)));
        // A bye week in the data: week 4's poll still ends at week 2.
        assert_eq!(snapshot_for_poll(&snaps, w(4)).week, Some(w(2)));
        for n in 1..=6 {
            let s = snapshot_for_poll(&snaps, w(n));
            assert!(s.week.is_none_or(|x| x < w(n)), "lookahead at week {n}");
        }
        let final_poll = Week {
            postseason: true,
            number: 1,
        };
        assert_eq!(snapshot_for_poll(&snaps, final_poll).week, Some(w(4)));
    }

    #[test]
    fn rank_ranges_bracket_point_rank() {
        let s = snap(Some(1), &[10.0, 9.5, 0.0, -10.0]);
        let r = ranks_with_range(&s, &[0, 1, 2, 3], 2000, 1);
        assert_eq!(r[0].0, 1);
        assert_eq!(r[3], (4, 4, 4), "a clearly worst team has no range");
        assert!(r[0].1 == 1 && r[0].2 == 2, "close teams swap: {:?}", r[0]);
    }

    #[test]
    fn gap_sign() {
        let c = Comparison {
            year: 2024,
            poll: "AP Top 25".into(),
            week: Some(5),
            team: "X".into(),
            conference: "SEC".into(),
            poll_rank: 5,
            model_rank: 15,
            model_low: 10,
            model_high: 20,
            preseason_ap: Some(3),
        };
        assert_eq!(c.gap(), 10.0);
        assert!(c.poll_above_range());
        assert!(!c.poll_below_range());
    }
}
