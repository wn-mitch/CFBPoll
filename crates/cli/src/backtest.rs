//! `cfbpoll backtest`: out-of-sample forecast quality per season against
//! baselines. Every forecast is made before the game it scores.

use crate::load::load_season;
use crate::prior::{load_model, loso_priors, union_years, PriorData};
use anyhow::{Context, Result};
use cfbd::Cache;
use model::data::{Class, Game, Season};
use model::layout::TeamPrior;
use model::legacy_elo::{self, EloParams, EloTable};
use model::metrics::{Forecast, Scorecard, Summary};
use model::params::Params;
use model::season::{run_season, RunOptions};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Which games a scorecard covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Slice {
    /// Both teams FBS.
    Fbs,
    /// Both teams FBS, regular-season weeks 1–4.
    FbsEarly,
    /// Any game between two rated (FBS or FCS) teams.
    Rated,
}

impl Slice {
    const ALL: [Slice; 3] = [Slice::Fbs, Slice::FbsEarly, Slice::Rated];

    fn contains(self, season: &Season, g: &Game) -> bool {
        let (h, a) = (season.teams[g.home].class, season.teams[g.away].class);
        let fbs = h == Class::Fbs && a == Class::Fbs;
        match self {
            Slice::Fbs => fbs,
            Slice::FbsEarly => fbs && !g.week.postseason && g.week.number <= 4,
            Slice::Rated => h.is_rated() && a.is_rated(),
        }
    }
}

pub type Cards = BTreeMap<(String, Slice), Scorecard>;

fn add(cards: &mut Cards, season: &Season, model: &str, g: &Game, forecast: Forecast) {
    for slice in Slice::ALL {
        if slice.contains(season, g) {
            cards
                .entry((model.to_string(), slice))
                .or_default()
                .add(&forecast);
        }
    }
}

fn forecast(g: &Game, home_win: f64, margin: Option<f64>, total: Option<f64>) -> Option<Forecast> {
    let (h, a) = g.score?;
    Some(Forecast {
        home_win,
        margin,
        total,
        actual_margin: h - a,
        actual_total: h + a,
        market_margin: g.market_margin,
        market_total: g.market_total,
    })
}

/// Scores the Kalman model on one season.
pub fn score_model(
    season: &Season,
    params: &Params,
    priors: &[Option<TeamPrior>],
    name: &str,
    cards: &mut Cards,
) {
    let run = run_season(
        season,
        params,
        &RunOptions {
            priors,
            before: None,
        },
    );
    for (gi, p) in &run.predictions {
        let g = &season.games[*gi];
        if let Some(f) = forecast(g, p.home_win, Some(p.margin), Some(p.total)) {
            add(cards, season, name, g, f);
        }
    }
}

pub fn run(
    cache: &Cache,
    years: &[i32],
    params: &Params,
    prior: Option<&Path>,
    out: Option<&Path>,
) -> Result<()> {
    let seasons: Vec<Season> = years
        .iter()
        .map(|&y| load_season(cache, y))
        .collect::<Result<_>>()?;
    let priors: Option<Vec<Vec<Option<TeamPrior>>>> = match prior {
        None => None,
        Some(path) => {
            let template = load_model(path)?;
            let data = PriorData::load(cache, &union_years(years, &template.train_years), params)?;
            Some(loso_priors(&data, &template, years))
        }
    };
    let score_only = Params {
        use_efficiency: false,
        ..params.clone()
    };

    let mut by_year: BTreeMap<i32, Cards> = BTreeMap::new();
    let mut elo_table: Option<EloTable> = None;
    let elo = EloParams::default();
    for (si, season) in seasons.iter().enumerate() {
        let cards = by_year.entry(season.year).or_default();
        score_model(season, params, &[], "kalman", cards);
        if params.use_efficiency {
            score_model(season, &score_only, &[], "kalman-score", cards);
        }
        if let Some(p) = &priors {
            score_model(season, params, &p[si], "kalman-prior", cards);
        }

        let (preds, table) = legacy_elo::run(season, &elo, elo_table.as_ref());
        for (gi, p, m) in preds {
            let g = &season.games[gi];
            if let Some(f) = forecast(g, p, Some(m), None) {
                add(cards, season, "legacy-elo", g, f);
            }
        }
        elo_table = Some(table);

        for g in &season.games {
            let Some((he, ae)) = g.cfbd_elo else { continue };
            let hfa = if g.neutral { 0.0 } else { CFBD_ELO_HFA };
            if let Some(f) = forecast(g, legacy_elo::expected(he + hfa - ae), None, None) {
                add(cards, season, "cfbd-elo", g, f);
            }
        }
    }

    let mut total: Cards = BTreeMap::new();
    for cards in by_year.values() {
        for (k, c) in cards {
            total.entry(k.clone()).or_default().merge(c);
        }
    }

    if let Some(path) = out {
        #[derive(Serialize)]
        struct Row {
            model: String,
            slice: Slice,
            summary: Summary,
        }
        #[derive(Serialize)]
        struct Accuracy {
            years: String,
            rows: Vec<Row>,
        }
        let acc = Accuracy {
            years: format!("{}–{}", years[0], years[years.len() - 1]),
            rows: total
                .iter()
                .map(|((m, s), c)| Row {
                    model: m.clone(),
                    slice: *s,
                    summary: c.summary(),
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&acc)?;
        std::fs::write(path, text + "\n").with_context(|| format!("writing {}", path.display()))?;
        eprintln!("wrote {}", path.display());
    }
    for (year, cards) in &by_year {
        println!("\n== {year}");
        print_table(cards, &[Slice::Fbs]);
    }
    println!("\n== {}-{} combined", years[0], years[years.len() - 1]);
    print_table(&total, &Slice::ALL);
    Ok(())
}

/// Home-field advantage, in Elo points, applied to CFBD's pregame Elo
/// baseline. CFBD does not publish one; 55 is FiveThirtyEight's college value.
const CFBD_ELO_HFA: f64 = 55.0;

fn print_table(cards: &Cards, slices: &[Slice]) {
    println!(
        "{:<12} {:<9} {:>6} {:>8} {:>7} {:>6} {:>8} {:>8} {:>8}",
        "model", "slice", "games", "logloss", "brier", "acc", "mrg_mae", "vs_mkt", "mkt_mae"
    );
    for ((model, slice), card) in cards {
        if !slices.contains(slice) {
            continue;
        }
        let s: Summary = card.summary();
        println!(
            "{:<12} {:<9} {:>6} {:>8.4} {:>7.4} {:>6.3} {:>8.2} {:>8.2} {:>8.2}",
            model,
            format!("{slice:?}"),
            s.games,
            s.log_loss,
            s.brier,
            s.accuracy,
            s.margin_mae,
            s.model_mae_on_market_games,
            s.market_mae
        );
    }
}
