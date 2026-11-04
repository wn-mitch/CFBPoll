# CFB Poll

A computer poll that ranks college football teams by how good they are:
the margin each would beat an average FBS team by on a neutral field, with
the uncertainty shown. Live at <https://cfb.alpacasoft.dev>.

## How it works

- **Ratings.** Each FBS and FCS team has an offense and a defense rating, in
  points. A Kalman filter updates them after every game from both scores and
  each offense's garbage-time-excluded efficiency (PPA per play). Conference
  levels start equal each season and move only on cross-conference games.
- **Season start.** Last season's rating, adjusted for returning production,
  roster continuity and transfer-portal flow by position group, fit by ridge
  regression (`artifacts/prior.json`).
- **Odds.** Monte Carlo simulation of the remaining schedule, conference
  title games and the 12-team playoff.
- **Poll audit.** Compares the AP, Coaches and committee rankings with the
  model using only games played before each release.

Hyperparameters are fit on 2010–2023 and evaluated on 2024–2025
(`artifacts/accuracy.json`). The method page on the site has the details.

## Usage

Requires Rust (see `rust-toolchain.toml`), Node 24+, `just`, and a free
[CollegeFootballData](https://collegefootballdata.com/key) API key in
`CFBD_API_KEY` or `.env`.

```sh
just fetch-all      # cache every season under data/raw (about 190 API calls)
just dev            # build site data for the current season and serve the site
just test           # Rust and site tests
```

Model maintenance (slow; outputs are committed under `artifacts/`):

```sh
just prior          # refit the roster-churn prior
just fit            # refit hyperparameters
just backtest       # held-out accuracy
just audit          # human-poll comparison
```

`cargo run -p cfbpoll -- --help` lists the CLI, including `rank` and `odds`
for terminal output.

## Deployment

`.github/workflows/update.yml` refreshes the current season, rebuilds and
deploys to Cloudflare Workers four mornings a week during the season. It
needs the `CFBD_API_KEY`, `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`
repository secrets.
