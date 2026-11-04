//! Conference standings, tiebreakers, playoff selection, and the bracket.
//!
//! Tiebreakers are simplified relative to each conference's own rules:
//! conference win percentage, then head-to-head win percentage among the
//! tied teams, then the model's rating.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CFP rules for one season.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayoffRules {
    pub field_size: usize,
    /// Conferences whose champion qualifies automatically.
    pub auto_conferences: Vec<String>,
    /// Conferences whose highest-ranked champion takes one automatic bid.
    pub group_conferences: Vec<String>,
    /// Independent that qualifies automatically when ranked within the field size.
    pub ranked_independent: Option<String>,
    /// Number of top seeds with a first-round bye.
    pub byes: usize,
}

impl PlayoffRules {
    /// 2026: 12 teams seeded straight by ranking; Power Four champions, the
    /// highest-ranked other FBS champion, and Notre Dame if ranked in the top
    /// 12 qualify automatically.
    pub fn cfp_2026() -> Self {
        Self {
            field_size: 12,
            auto_conferences: ["ACC", "Big Ten", "Big 12", "SEC"]
                .map(String::from)
                .to_vec(),
            group_conferences: [
                "American Athletic",
                "Conference USA",
                "Mid-American",
                "Mountain West",
                "Pac-12",
                "Sun Belt",
            ]
            .map(String::from)
            .to_vec(),
            ranked_independent: Some("Notre Dame".into()),
            byes: 4,
        }
    }
}

/// Result of one conference game for standings: `(winner, loser)`.
pub type Result2 = (usize, usize);

/// Orders a conference's teams from first to last.
///
/// `results` are that conference's games; `rating` breaks remaining ties.
pub fn standings(
    teams: &[usize],
    results: &[Result2],
    rating: &dyn Fn(usize) -> f64,
) -> Vec<usize> {
    let mut wins: HashMap<usize, f64> = teams.iter().map(|&t| (t, 0.0)).collect();
    let mut games: HashMap<usize, f64> = teams.iter().map(|&t| (t, 0.0)).collect();
    for &(w, l) in results {
        if let (Some(_), Some(_)) = (wins.get(&w), wins.get(&l)) {
            *wins.get_mut(&w).unwrap() += 1.0;
            *games.get_mut(&w).unwrap() += 1.0;
            *games.get_mut(&l).unwrap() += 1.0;
        }
    }
    let pct = |t: usize| {
        if games[&t] > 0.0 {
            wins[&t] / games[&t]
        } else {
            0.0
        }
    };
    let mut order = teams.to_vec();
    order.sort_by(|&a, &b| {
        pct(b)
            .total_cmp(&pct(a))
            .then(rating(b).total_cmp(&rating(a)))
    });

    // Re-sort each block of teams tied on win percentage by head-to-head
    // results among themselves, then rating.
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && pct(order[j]) == pct(order[i]) {
            j += 1;
        }
        if j - i > 1 {
            let tied: Vec<usize> = order[i..j].to_vec();
            let mut h2h_w: HashMap<usize, f64> = tied.iter().map(|&t| (t, 0.0)).collect();
            let mut h2h_g: HashMap<usize, f64> = tied.iter().map(|&t| (t, 0.0)).collect();
            for &(w, l) in results {
                if h2h_w.contains_key(&w) && h2h_w.contains_key(&l) {
                    *h2h_w.get_mut(&w).unwrap() += 1.0;
                    *h2h_g.get_mut(&w).unwrap() += 1.0;
                    *h2h_g.get_mut(&l).unwrap() += 1.0;
                }
            }
            // Teams that did not play every other tied team get no
            // head-to-head credit, so head-to-head only separates complete
            // round robins among the tied group.
            let full = tied.len() as f64 - 1.0;
            let h2h = |t: usize| {
                if h2h_g[&t] >= full {
                    h2h_w[&t] / h2h_g[&t]
                } else {
                    0.5
                }
            };
            order[i..j].sort_by(|&a, &b| {
                h2h(b)
                    .total_cmp(&h2h(a))
                    .then(rating(b).total_cmp(&rating(a)))
            });
        }
        i = j;
    }
    order
}

/// Selects the playoff field.
///
/// `ranking` is FBS teams best-first. `champions` maps conference name to
/// champion. Returns the field ordered by seed.
pub fn select_field(
    rules: &PlayoffRules,
    ranking: &[usize],
    champions: &HashMap<String, usize>,
    team_name: &dyn Fn(usize) -> String,
) -> Vec<usize> {
    let rank_of: HashMap<usize, usize> = ranking.iter().enumerate().map(|(i, &t)| (t, i)).collect();
    let rank = |t: &usize| rank_of.get(t).copied().unwrap_or(usize::MAX);
    let mut field: Vec<usize> = Vec::new();
    for c in &rules.auto_conferences {
        if let Some(&t) = champions.get(c) {
            field.push(t);
        }
    }
    if let Some(t) = rules
        .group_conferences
        .iter()
        .filter_map(|c| champions.get(c))
        .min_by_key(|t| rank(t))
    {
        field.push(*t);
    }
    if let Some(ind) = &rules.ranked_independent {
        if let Some(&t) = ranking
            .iter()
            .take(rules.field_size)
            .find(|&&t| &team_name(t) == ind)
        {
            field.push(t);
        }
    }
    field.sort_by_key(|t| rank(t));
    field.dedup();
    for &t in ranking {
        if field.len() >= rules.field_size {
            break;
        }
        if !field.contains(&t) {
            field.push(t);
        }
    }
    field.sort_by_key(|t| rank(t));
    field.truncate(rules.field_size);
    field
}

/// Plays a bracket seeded `field[0]` (1) through `field[11]` (12). First
/// round games are at the higher seed; later rounds are neutral.
/// `play(home, away, neutral)` returns the winner. Returns `(winner,
/// quarterfinalists, semifinalists, finalists)`.
pub fn play_bracket(
    field: &[usize],
    byes: usize,
    play: &mut dyn FnMut(usize, usize, bool) -> usize,
) -> (usize, Vec<usize>, Vec<usize>, Vec<usize>) {
    assert_eq!(field.len(), 12, "bracket supports a 12-team field");
    assert_eq!(byes, 4);
    // First round: 5v12, 6v11, 7v10, 8v9.
    let r1: Vec<usize> = (0..4)
        .map(|i| play(field[4 + i], field[11 - i], false))
        .collect();
    // Quarterfinals: 1 v 8/9, 2 v 7/10, 3 v 6/11, 4 v 5/12.
    let qf_pairs = [
        (field[0], r1[3]),
        (field[1], r1[2]),
        (field[2], r1[1]),
        (field[3], r1[0]),
    ];
    let quarter: Vec<usize> = qf_pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
    let qf: Vec<usize> = qf_pairs.iter().map(|&(a, b)| play(a, b, true)).collect();
    // Semifinals: (1 side) v (4 side), (2 side) v (3 side).
    let sf = [play(qf[0], qf[3], true), play(qf[1], qf[2], true)];
    let champion = play(sf[0], sf[1], true);
    (champion, quarter, qf, sf.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(t: usize) -> f64 {
        -(t as f64)
    }

    #[test]
    fn two_way_tie_uses_head_to_head() {
        // 1 and 2 both 2-1 in conference; 2 beat 1.
        let results = vec![(2, 1), (1, 3), (1, 4), (2, 3), (4, 2), (3, 4)];
        let order = standings(&[1, 2, 3, 4], &results, &rating);
        assert_eq!(&order[..2], &[2, 1], "head-to-head beats rating");
    }

    #[test]
    fn three_way_round_robin_tie_falls_to_rating() {
        // 1>2, 2>3, 3>1: each 1-1 head-to-head.
        let results = vec![(1, 2), (2, 3), (3, 1)];
        assert_eq!(standings(&[3, 2, 1], &results, &rating), vec![1, 2, 3]);
    }

    #[test]
    fn four_way_tie_with_incomplete_head_to_head_uses_rating() {
        // Four teams 1-1; not everyone played everyone.
        let results = vec![(4, 1), (1, 2), (2, 3), (3, 4)];
        assert_eq!(
            standings(&[4, 3, 2, 1], &results, &rating),
            vec![1, 2, 3, 4]
        );
    }

    fn name(t: usize) -> String {
        if t == 99 {
            "Notre Dame".into()
        } else {
            format!("T{t}")
        }
    }

    #[test]
    fn field_includes_auto_bids_and_seeds_by_rank() {
        let rules = PlayoffRules::cfp_2026();
        // Ranking 0..30 plus Notre Dame at 11th; teams 40 and 41 are low-ranked champions.
        let mut ranking: Vec<usize> = (0..30).collect();
        ranking.insert(10, 99);
        ranking.push(40);
        ranking.push(41);
        let mut champs = HashMap::new();
        champs.insert("SEC".to_string(), 0);
        champs.insert("Big Ten".to_string(), 1);
        champs.insert("ACC".to_string(), 40);
        champs.insert("Big 12".to_string(), 5);
        champs.insert("Sun Belt".to_string(), 41);
        champs.insert("Mountain West".to_string(), 25);
        let field = select_field(&rules, &ranking, &champs, &name);
        assert_eq!(field.len(), 12);
        for t in [0, 1, 40, 5, 99] {
            assert!(field.contains(&t), "missing auto bid {t}");
        }
        assert!(
            field.contains(&25),
            "highest-ranked group champion qualifies"
        );
        assert!(!field.contains(&41), "only one group champion qualifies");
        // Seeds follow ranking: the ACC champion ranked last is seeded 12th.
        assert_eq!(field[11], 40);
        assert_eq!(field[0], 0);
    }

    #[test]
    fn notre_dame_outside_top_twelve_gets_no_auto_bid() {
        let rules = PlayoffRules::cfp_2026();
        let mut ranking: Vec<usize> = (0..30).collect();
        ranking.insert(14, 99);
        let field = select_field(&rules, &ranking, &HashMap::new(), &name);
        assert!(!field.contains(&99));
        assert_eq!(field, (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn bracket_favorites_win_when_higher_seed_always_wins() {
        let field: Vec<usize> = (0..12).collect();
        let mut first_round_homes = Vec::new();
        let mut play = |h: usize, a: usize, neutral: bool| {
            if !neutral {
                first_round_homes.push(h);
            }
            h.min(a)
        };
        let (champ, quarter, semis, finals) = play_bracket(&field, 4, &mut play);
        assert_eq!(champ, 0);
        assert_eq!(first_round_homes, vec![4, 5, 6, 7], "seeds 5-8 host");
        assert_eq!(quarter.len(), 8);
        assert_eq!(semis, vec![0, 1, 2, 3]);
        assert_eq!(finals, vec![0, 1]);
    }
}
