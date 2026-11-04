//! Simulation invariants on a synthetic five-conference season.

use model::data::{Class, Game, Season, Team, Week};
use model::params::Params;
use model::playoff::PlayoffRules;
use model::season::{run_season, RunOptions};
use model::sim::{simulate, SimOptions};

const CONFS: [&str; 5] = ["SEC", "Big Ten", "ACC", "Big 12", "Sun Belt"];

fn season(championship_played: bool) -> Season {
    let mut teams = Vec::new();
    for c in CONFS {
        for i in 0..4 {
            teams.push(Team {
                name: format!("{c} {i}"),
                class: Class::Fbs,
                conference: Some(c.into()),
            });
        }
    }
    let mut games = Vec::new();
    let mut id = 0;
    let mut add =
        |week: i32, h: usize, a: usize, score: Option<(f64, f64)>, conf: bool, champ: bool| {
            id += 1;
            games.push(Game {
                id,
                week: Week {
                    postseason: false,
                    number: week,
                },
                day: 20_000 + 7 * week as i64,
                neutral: champ,
                conference_game: conf,
                championship: champ,
                home: h,
                away: a,
                score,
                ppa: None,
                market_margin: None,
                market_total: None,
                cfbd_elo: None,
            });
        };
    // Weeks 1-3: completed round robin inside each conference; lower index
    // wins. Week 4: unplayed cross-conference games.
    for c in 0..5 {
        let b = c * 4;
        let rr = [(0, 1), (2, 3), (0, 2), (1, 3), (0, 3), (1, 2)];
        for (k, (x, y)) in rr.iter().enumerate() {
            add(
                1 + k as i32 / 2,
                b + x,
                b + y,
                Some((31.0, 17.0)),
                true,
                false,
            );
        }
    }
    for c in 0..5 {
        let other = (c + 1) % 5;
        for i in 0..4 {
            add(4, c * 4 + i, other * 4 + (3 - i), None, false, false);
        }
    }
    if championship_played {
        // SEC 1 upsets SEC 0 in a completed title game.
        add(5, 0, 1, Some((10.0, 24.0)), true, true);
    }
    Season {
        year: 2026,
        teams,
        games,
    }
}

#[test]
fn simulation_invariants() {
    let s = season(false);
    let params = Params::default();
    let run = run_season(&s, &params, &RunOptions::default());
    let rules = PlayoffRules::cfp_2026();
    let opts = SimOptions {
        draws: 4000,
        seed: 11,
    };
    let a = simulate(&s, &run, &params, &rules, opts);
    let b = simulate(&s, &run, &params, &rules, opts);
    let odds: Vec<_> = a.teams.iter().map(|t| t.clone().unwrap()).collect();

    // Reproducible from the seed.
    for (x, y) in a.teams.iter().zip(&b.teams) {
        let (x, y) = (x.as_ref().unwrap(), y.as_ref().unwrap());
        assert_eq!(x.title, y.title);
        assert_eq!(x.rank_p50, y.rank_p50);
    }

    for c in 0..5 {
        let members = &odds[c * 4..c * 4 + 4];
        let title: f64 = members.iter().map(|o| o.conf_title).sum();
        assert!(
            (title - 1.0).abs() < 1e-9,
            "conference {c} title odds sum to {title}"
        );
        let game: f64 = members.iter().map(|o| o.conf_title_game).sum();
        assert!(
            (game - 2.0).abs() < 1e-9,
            "two teams per title game, got {game}"
        );
    }
    let field: f64 = odds.iter().map(|o| o.playoff).sum();
    assert!((field - 12.0).abs() < 1e-9, "playoff odds sum to {field}");
    let titles: f64 = odds.iter().map(|o| o.title).sum();
    assert!((titles - 1.0).abs() < 1e-9);
    let byes: f64 = odds.iter().map(|o| o.bye).sum();
    assert!((byes - 4.0).abs() < 1e-9);

    for o in &odds {
        assert!(o.rank_p05 <= o.rank_p50 && o.rank_p50 <= o.rank_p95);
        assert!(o.title <= o.final_ + 1e-12 && o.final_ <= o.semifinal + 1e-12);
        assert!(o.semifinal <= o.quarterfinal + 1e-12 && o.bye <= o.playoff + 1e-12);
        // Three completed conference games and two remaining cross-conference
        // games, plus at most one title game.
        let games = o.expected_wins + o.expected_losses;
        assert!((5.0..=6.0 + 1e-9).contains(&games), "games {games}");
    }
    // A 3-0 conference leader must out-rank a 0-3 team in title odds.
    assert!(odds[0].conf_title > odds[3].conf_title);
    // Every Power Four champion is in the field, and one Sun Belt team gets
    // the group bid, so every conference sends at least one team.
    for c in 0..5 {
        let sent: f64 = odds[c * 4..c * 4 + 4].iter().map(|o| o.playoff).sum();
        assert!(sent >= 1.0 - 1e-9, "conference {c} sent {sent}");
    }
}

#[test]
fn completed_championship_is_final() {
    let s = season(true);
    let params = Params::default();
    let run = run_season(&s, &params, &RunOptions::default());
    let r = simulate(
        &s,
        &run,
        &params,
        &PlayoffRules::cfp_2026(),
        SimOptions {
            draws: 500,
            seed: 3,
        },
    );
    let sec1 = r.teams[1].as_ref().unwrap();
    assert_eq!(
        sec1.conf_title, 1.0,
        "the actual winner is champion in every draw"
    );
    assert_eq!(r.teams[0].as_ref().unwrap().conf_title, 0.0);
}
