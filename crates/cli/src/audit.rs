//! `cfbpoll audit`: compare the AP, Coaches and committee rankings with
//! this model, week by week, using only games played before each release.

use crate::load::load_season;
use crate::prior::{load_model, loso_priors, union_years, PriorData};
use anyhow::{Context, Result};
use cfbd::{Cache, Dataset, PollWeek};
use model::bias::{
    group_stat, preseason_bucket, ranks_with_range, snapshot_for_poll, Comparison, GroupStat,
};
use model::data::{Class, Week};
use model::layout::TeamPrior;
use model::params::Params;
use model::season::{run_season, RunOptions};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const POLLS: [&str; 3] = ["AP Top 25", "Coaches Poll", "Playoff Committee Rankings"];
const AP: &str = "AP Top 25";

#[derive(Serialize)]
struct Snub {
    key: String,
    /// Model top-10 teams left out of the AP top 25, counted per poll week.
    count: usize,
}

#[derive(Serialize)]
struct Audit {
    seasons: Vec<i32>,
    comparisons: usize,
    /// Per poll, per conference: positive mean gap means the poll ranks the
    /// conference's teams better than the model does.
    by_conference: BTreeMap<String, Vec<GroupStat>>,
    /// AP poll, by preseason-AP bucket and poll week.
    anchoring: Vec<(String, Vec<(i32, GroupStat)>)>,
    /// AP poll mean absolute gap per week.
    agreement_by_week: Vec<(i32, f64)>,
    snubs_by_conference: Vec<Snub>,
    /// The latest AP release of the latest season, for the current table.
    latest: Vec<Comparison>,
}

fn poll_week(p: &PollWeek) -> Week {
    Week {
        postseason: p.season_type == "postseason",
        number: p.week,
    }
}

pub fn run(
    cache: &Cache,
    years: &[i32],
    params: &Params,
    prior: Option<&Path>,
    out: &Path,
) -> Result<()> {
    let priors: Vec<Vec<Option<TeamPrior>>> = match prior {
        None => vec![Vec::new(); years.len()],
        Some(path) => {
            let template = load_model(path)?;
            let data = PriorData::load(cache, &union_years(years, &template.train_years), params)?;
            loso_priors(&data, &template, years)
        }
    };

    let mut rows: Vec<Comparison> = Vec::new();
    let mut snubs: HashMap<String, usize> = HashMap::new();
    for (yi, &year) in years.iter().enumerate() {
        let season = load_season(cache, year)?;
        let run = run_season(
            &season,
            params,
            &RunOptions {
                priors: &priors[yi],
                before: None,
            },
        );
        let fbs: Vec<usize> = (0..season.teams.len())
            .filter(|&t| season.teams[t].class == Class::Fbs)
            .collect();
        let index: HashMap<&str, usize> = fbs
            .iter()
            .map(|&t| (season.teams[t].name.as_str(), t))
            .collect();
        let mut weeks = cache.load::<PollWeek>(year, Dataset::Rankings)?;
        weeks.sort_by_key(poll_week);
        let preseason: HashMap<String, u32> = weeks
            .iter()
            .find(|w| !w.season_type.contains("post") && w.week == 1)
            .and_then(|w| w.polls.iter().find(|p| p.poll == AP))
            .map(|p| {
                p.ranks
                    .iter()
                    .map(|r| (r.school.clone(), r.rank as u32))
                    .collect()
            })
            .unwrap_or_default();

        for pw in &weeks {
            if pw.season_type != "regular" && pw.season_type != "postseason" {
                continue;
            }
            let week = poll_week(pw);
            let snap = snapshot_for_poll(&run.snapshots, week);
            let ranks = ranks_with_range(snap, &fbs, 400, (year as u64) << 8 | pw.week as u64);
            let rank_of: HashMap<usize, (usize, usize, usize)> =
                fbs.iter().copied().zip(ranks).collect();
            for poll in pw.polls.iter().filter(|p| POLLS.contains(&p.poll.as_str())) {
                for r in &poll.ranks {
                    let Some(&t) = index.get(r.school.as_str()) else {
                        continue;
                    };
                    let (model_rank, model_low, model_high) = rank_of[&t];
                    rows.push(Comparison {
                        year,
                        poll: poll.poll.clone(),
                        week: (!week.postseason).then_some(week.number),
                        team: r.school.clone(),
                        conference: season.teams[t]
                            .conference
                            .clone()
                            .unwrap_or_else(|| "Independent".into()),
                        poll_rank: r.rank as u32,
                        model_rank,
                        model_low,
                        model_high,
                        preseason_ap: preseason.get(&r.school).copied(),
                    });
                }
                if poll.poll == AP && week.number > 1 {
                    let in_poll: Vec<&str> = poll.ranks.iter().map(|r| r.school.as_str()).collect();
                    for (&t, &(rank, _, _)) in &rank_of {
                        if rank <= 10 && !in_poll.contains(&season.teams[t].name.as_str()) {
                            let c = season.teams[t]
                                .conference
                                .clone()
                                .unwrap_or_else(|| "Independent".into());
                            *snubs.entry(c).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    let mut by_conference: BTreeMap<String, Vec<GroupStat>> = BTreeMap::new();
    for poll in POLLS {
        let mut groups: BTreeMap<&str, Vec<&Comparison>> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.poll == poll) {
            groups.entry(r.conference.as_str()).or_default().push(r);
        }
        let mut stats: Vec<GroupStat> = groups
            .iter()
            .filter(|(_, v)| v.len() >= 20)
            .map(|(k, v)| group_stat(k, v))
            .collect();
        stats.sort_by(|a, b| b.mean_gap.total_cmp(&a.mean_gap));
        by_conference.insert(poll.to_string(), stats);
    }

    let ap: Vec<&Comparison> = rows
        .iter()
        .filter(|r| r.poll == AP && r.week.is_some())
        .collect();
    let mut anchoring = Vec::new();
    for bucket in [
        "preseason 1-5",
        "preseason 6-10",
        "preseason 11-25",
        "unranked preseason",
    ] {
        let mut by_week: BTreeMap<i32, Vec<&Comparison>> = BTreeMap::new();
        for r in ap
            .iter()
            .filter(|r| preseason_bucket(r.preseason_ap) == bucket)
        {
            by_week.entry(r.week.unwrap()).or_default().push(r);
        }
        let series = by_week
            .iter()
            .map(|(w, v)| (*w, group_stat(bucket, v)))
            .collect();
        anchoring.push((bucket.to_string(), series));
    }
    let mut abs_by_week: BTreeMap<i32, Vec<f64>> = BTreeMap::new();
    for r in &ap {
        abs_by_week
            .entry(r.week.unwrap())
            .or_default()
            .push(r.gap().abs());
    }
    let agreement_by_week = abs_by_week
        .into_iter()
        .map(|(w, v)| (w, v.iter().sum::<f64>() / v.len() as f64))
        .collect();
    let mut snubs_by_conference: Vec<Snub> = snubs
        .into_iter()
        .map(|(key, count)| Snub { key, count })
        .collect();
    snubs_by_conference.sort_by(|a, b| b.count.cmp(&a.count).then(a.key.cmp(&b.key)));

    let last_year = *years.last().expect("at least one season");
    let latest_week = rows
        .iter()
        .filter(|r| r.year == last_year && r.poll == AP)
        .map(|r| r.week.unwrap_or(i32::MAX))
        .max();
    let latest: Vec<Comparison> = rows
        .iter()
        .filter(|r| {
            r.year == last_year && r.poll == AP && Some(r.week.unwrap_or(i32::MAX)) == latest_week
        })
        .cloned()
        .collect();

    let audit = Audit {
        seasons: years.to_vec(),
        comparisons: rows.len(),
        by_conference,
        anchoring,
        agreement_by_week,
        snubs_by_conference,
        latest,
    };
    print_summary(&audit);
    let json = serde_json::to_string_pretty(&audit)?;
    std::fs::write(out, json + "\n").with_context(|| format!("writing {}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

fn print_summary(a: &Audit) {
    println!(
        "{} comparisons over {} seasons",
        a.comparisons,
        a.seasons.len()
    );
    for (poll, stats) in &a.by_conference {
        println!("\n{poll}: mean (model rank - poll rank); positive = poll ranks them higher");
        for s in stats {
            println!(
                "  {:<20} n={:>5} mean {:>6.2} median {:>5.1} poll above model 90% range {:>5.1}%",
                s.key,
                s.n,
                s.mean_gap,
                s.median_gap,
                100.0 * s.above_range
            );
        }
    }
    println!("\nAP anchoring (mean gap by preseason rank, selected weeks):");
    for (bucket, series) in &a.anchoring {
        let pick: Vec<String> = series
            .iter()
            .filter(|(w, _)| [2, 5, 8, 11, 14].contains(w))
            .map(|(w, s)| format!("wk{w} {:+.1}", s.mean_gap))
            .collect();
        println!("  {bucket:<20} {}", pick.join("  "));
    }
}
