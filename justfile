# Training seasons for fitting; the two most recent complete seasons are held out.
train := "2010-2023"
holdout := "2024-2025"
cli := "cargo run --release --quiet -p cfbpoll --"

default:
    @just --list

# Fetch the current season (or YEARS) from CFBD into data/raw.
fetch years="":
    {{cli}} fetch {{ if years == "" { "" } else { "--years " + years } }}

# Fetch every season the model uses.
fetch-all:
    {{cli}} fetch --years 2010-2026

# Fit hyperparameters (slow). Writes artifacts/params.json.
fit:
    {{cli}} fit --years {{train}} --params artifacts/params.json --prior artifacts/prior.json --out artifacts/params.json

# Refit the roster-churn prior. Writes artifacts/prior.json.
prior:
    {{cli}} prior --years 2011-2023 --params artifacts/params.json --out artifacts/prior.json

# Score held-out seasons against baselines. Writes artifacts/accuracy.json.
backtest years=holdout:
    {{cli}} backtest --years {{years}} --params artifacts/params.json --prior artifacts/prior.json --out artifacts/accuracy.json

# Compare human polls with the model. Writes artifacts/bias.json.
audit:
    {{cli}} audit --years 2014-2026 --params artifacts/params.json --prior artifacts/prior.json --out artifacts/bias.json

# Write site JSON for the current season.
build-data:
    {{cli}} build --out site/static/data

dev: build-data
    cd site && npm run dev

test:
    cargo test --workspace
    cd site && npm test && npm run check

lint:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --check

# Build the static site from current data.
site: build-data
    cd site && npm run build

# Regenerate the Open Graph image from the built site.
og: site
    cd site && npm run og

deploy: test site
    cd site && npx wrangler deploy
