//! Season-level behavior on synthetic schedules.

use model::data::{Class, Game, Season, Team, Week};
use model::kalman::cholesky;
use model::layout::Layout;
use model::observe::predict;
use model::params::Params;
use model::season::{run_season, RunOptions};

fn team(name: &str, class: Class, conf: Option<&str>) -> Team {
    Team {
        name: name.into(),
        class,
        conference: conf.map(Into::into),
    }
}

fn game(id: i64, week: i32, home: usize, away: usize, score: Option<(f64, f64)>) -> Game {
    Game {
        id,
        week: Week {
            postseason: false,
            number: week,
        },
        day: 20_000 + 7 * week as i64,
        neutral: false,
        conference_game: false,
        home,
        away,
        score,
        ppa: None,
        market_margin: None,
        market_total: None,
        cfbd_elo: None,
    }
}

/// Deterministic pseudo-random stream (xorshift) so tests need no RNG crate.
struct Xs(u64);
impl Xs {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Approximately standard normal (sum of 12 uniforms).
    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.next_f64()).sum::<f64>() - 6.0
    }
}

#[test]
fn neutral_site_has_no_home_advantage() {
    let season = Season {
        year: 2024,
        teams: vec![team("A", Class::Fbs, None), team("B", Class::Fbs, None)],
        games: vec![],
    };
    let params = Params {
        hfa: 4.0,
        ..Params::default()
    };
    let run = run_season(&season, &params, &RunOptions::default());
    let mut g = game(1, 1, 0, 1, None);
    g.neutral = true;
    let neutral = predict(&run.state, &run.layout, &params, &g);
    assert!(neutral.margin.abs() < 1e-12);
    assert!((neutral.home_win - 0.5).abs() < 1e-12);
    g.neutral = false;
    let home = predict(&run.state, &run.layout, &params, &g);
    assert!((home.margin - 4.0).abs() < 1e-12);
}

#[test]
fn conference_levels_need_cross_conference_games() {
    // Two conferences, two teams each. Only in-conference games at first.
    let teams = vec![
        team("A1", Class::Fbs, Some("A")),
        team("A2", Class::Fbs, Some("A")),
        team("B1", Class::Fbs, Some("B")),
        team("B2", Class::Fbs, Some("B")),
    ];
    let inside = vec![
        game(1, 1, 0, 1, Some((35.0, 10.0))),
        game(2, 1, 2, 3, Some((21.0, 20.0))),
    ];
    let season = Season {
        year: 2024,
        teams: teams.clone(),
        games: inside.clone(),
    };
    let params = Params::default();
    let run = run_season(&season, &params, &RunOptions::default());
    let layout = Layout::new(&season);
    for c in 0..2 {
        // In-conference games say nothing about a conference's overall level
        // (co + cd), since both teams share it. Totals can still move its
        // scoring level (co − cd).
        let co = layout.conf_o(c);
        let cd = layout.conf_d(c);
        let combined = run.state.mean[co] + run.state.mean[cd];
        assert!(
            combined.abs() < 1e-9,
            "conference {c} overall moved to {combined}"
        );
    }

    // One cross-conference upset: conference B beats conference A by 30.
    let mut games = inside;
    games.push(game(3, 2, 0, 3, Some((10.0, 40.0))));
    let season = Season {
        year: 2024,
        teams,
        games,
    };
    let run = run_season(&season, &params, &RunOptions::default());
    let level = |c| run.state.mean[layout.conf_o(c)] + run.state.mean[layout.conf_d(c)];
    assert!(
        level(0) < -0.1,
        "conference A should drop, got {}",
        level(0)
    );
    assert!(level(1) > 0.1, "conference B should rise, got {}", level(1));
}

#[test]
fn lower_division_opponent_is_redrawn_each_game() {
    let teams = vec![
        team("A", Class::Fbs, None),
        team("B", Class::Fbs, None),
        team("Division II", Class::DivisionII, None),
    ];
    // A crushes the D-II pseudo-team; then B plays it. B's prediction must
    // not reflect what A's game taught about one particular D-II school.
    let season = Season {
        year: 2024,
        teams,
        games: vec![
            game(1, 1, 0, 2, Some((70.0, 0.0))),
            game(2, 2, 1, 2, Some((30.0, 20.0))),
        ],
    };
    let params = Params::default();
    let run = run_season(&season, &params, &RunOptions::default());
    let (_, second) = run.predictions[1];
    let expected = params.mu_mean + params.hfa / 2.0
        - params.division_ii.mean
        - (params.mu_mean - params.hfa / 2.0 + params.division_ii.mean);
    assert!(
        (second.margin - expected).abs() < 1e-9,
        "B vs D-II margin {} should come from the D-II prior alone ({expected})",
        second.margin
    );
}

#[test]
fn recovers_offense_and_defense_from_synthetic_season() {
    // 12 teams with known offense/defense play 20 double round robins
    // (220 games each), so the standard error per rating is about 0.5.
    let truth: Vec<(f64, f64)> = (0..12)
        .map(|i| {
            let o = (i as f64 - 5.5) * 1.6;
            let d = ((i * 7) % 12) as f64 - 5.5;
            (o, d)
        })
        .collect();
    let teams: Vec<Team> = (0..12)
        .map(|i| team(&format!("T{i}"), Class::Fbs, None))
        .collect();
    let params = Params {
        team_drift_sd: 0.0,
        mu_drift_sd: 0.0,
        margin_cap: f64::INFINITY,
        score_sd: 7.0,
        ..Params::default()
    };
    let mut rng = Xs(0x9e37_79b9_7f4a_7c15);
    let mut games = Vec::new();
    let mut id = 0;
    for round in 0..20 {
        for h in 0..12 {
            for a in 0..12 {
                if h == a {
                    continue;
                }
                id += 1;
                let mu = params.mu_mean;
                let hp = mu + truth[h].0 - truth[a].1 + params.hfa / 2.0 + 7.0 * rng.normal();
                let ap = mu + truth[a].0 - truth[h].1 - params.hfa / 2.0 + 7.0 * rng.normal();
                games.push(game(id, round + 1, h, a, Some((hp, ap))));
            }
        }
    }
    let season = Season {
        year: 2024,
        teams,
        games,
    };
    let run = run_season(&season, &params, &RunOptions::default());
    let r = run.final_ratings();
    // Offense and defense are identified only up to a shared shift absorbed
    // by μ; compare after centering each.
    let center = |v: Vec<f64>| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        v.into_iter().map(|x| x - m).collect::<Vec<_>>()
    };
    let est_o = center(r.iter().take(12).map(|x| x.offense).collect());
    let est_d = center(r.iter().take(12).map(|x| x.defense).collect());
    let true_o = center(truth.iter().map(|x| x.0).collect());
    let true_d = center(truth.iter().map(|x| x.1).collect());
    for i in 0..12 {
        assert!(
            (est_o[i] - true_o[i]).abs() < 1.5,
            "offense {i}: {} vs {}",
            est_o[i],
            true_o[i]
        );
        assert!(
            (est_d[i] - true_d[i]).abs() < 1.5,
            "defense {i}: {} vs {}",
            est_d[i],
            true_d[i]
        );
    }
    let n = run.state.dim();
    assert!(
        cholesky(&run.state.cov, n).is_some(),
        "covariance lost positive definiteness"
    );
}

#[test]
fn snapshots_respect_cutoff_week() {
    let teams = vec![team("A", Class::Fbs, None), team("B", Class::Fbs, None)];
    let season = Season {
        year: 2024,
        teams,
        games: vec![
            game(1, 1, 0, 1, Some((30.0, 10.0))),
            game(2, 2, 1, 0, Some((30.0, 10.0))),
            game(3, 3, 0, 1, Some((30.0, 10.0))),
        ],
    };
    let cutoff = Week {
        postseason: false,
        number: 3,
    };
    let run = run_season(
        &season,
        &Params::default(),
        &RunOptions {
            before: Some(cutoff),
            ..Default::default()
        },
    );
    assert_eq!(run.predictions.len(), 2);
    assert!(run
        .snapshots
        .iter()
        .all(|s| s.week.is_none_or(|w| w < cutoff)));
    assert_eq!(run.snapshots.len(), 3, "preseason + weeks 1 and 2");
}
