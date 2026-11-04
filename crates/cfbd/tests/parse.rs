//! Real (trimmed) API responses parse into the typed structs.

use cfbd::*;
use serde::de::DeserializeOwned;

fn parse<T: DeserializeOwned>(name: &str) -> Vec<T> {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<T> = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(!rows.is_empty(), "{name} fixture is empty");
    rows
}

#[test]
fn games() {
    let g: Vec<Game> = parse("games_fbs");
    assert!(g[0].score().is_some());
    let fcs: Vec<Game> = parse("games_fcs");
    assert!(
        fcs[0].away_classification.is_none(),
        "unclassified opponent parses as None"
    );
    let unplayed: Vec<Game> = parse("games_unplayed");
    assert!(unplayed[0].score().is_none());
}

#[test]
fn other_datasets() {
    parse::<TeamGamePpa>("ppa_fbs");
    parse::<BettingGame>("lines");
    let polls: Vec<PollWeek> = parse("rankings");
    assert!(!polls[0].polls.is_empty());
    parse::<ReturningProduction>("returning");
    parse::<PlayerTransfer>("portal");
    parse::<RosterPlayer>("roster");
    parse::<TeamTalent>("talent");
    parse::<CalendarWeek>("calendar");
}
