//! CollegeFootballData API client with a per-season on-disk cache.
//!
//! The model reads only from the cache (`load`), so backtests and rebuilds
//! run offline. `Client::sync` is the only code that spends API calls.

mod endpoint;
mod types;

pub use endpoint::Dataset;
pub use types::*;

use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const BASE_URL: &str = "https://api.collegefootballdata.com";

/// Largest response body accepted. Full-season rosters run to tens of MB.
const MAX_BODY_BYTES: u64 = 256 * 1024 * 1024;

/// On-disk cache rooted at e.g. `data/raw`.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, year: i32, ds: Dataset) -> PathBuf {
        self.root.join(year.to_string()).join(ds.file_name())
    }

    /// Age of the cached file, or `None` when it is absent.
    pub fn age(&self, year: i32, ds: Dataset) -> Option<Duration> {
        let modified = fs::metadata(self.path(year, ds)).ok()?.modified().ok()?;
        Some(
            SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default(),
        )
    }

    /// Writes `body` atomically so an interrupted fetch never leaves a
    /// truncated file that later parses as valid-but-partial data.
    pub fn store(&self, year: i32, ds: Dataset, body: &str) -> Result<()> {
        let path = self.path(year, ds);
        let dir = path.parent().expect("cache path has a parent");
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("renaming to {}", path.display()))?;
        Ok(())
    }

    /// Parses a cached dataset. A missing file is an error naming the
    /// `fetch` command that fills it, since silently returning no rows would
    /// look like a season with no games.
    pub fn load<T: DeserializeOwned>(&self, year: i32, ds: Dataset) -> Result<Vec<T>> {
        let path = self.path(year, ds);
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "missing {}; run `cfbpoll fetch --years {year}` first",
                path.display()
            )
        })?;
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Like `load`, but an absent file yields `None`. For datasets that do
    /// not exist for early seasons (e.g. the transfer portal before 2021).
    pub fn load_optional<T: DeserializeOwned>(
        &self,
        year: i32,
        ds: Dataset,
    ) -> Result<Option<Vec<T>>> {
        if !self.path(year, ds).exists() {
            return Ok(None);
        }
        self.load(year, ds).map(Some)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// When a cached file must be refetched.
#[derive(Debug, Clone, Copy)]
pub enum Refresh {
    /// Fetch only files that are absent.
    Missing,
    /// Also refetch in-season datasets older than this.
    OlderThan(Duration),
    /// Refetch everything requested.
    All,
}

/// Authenticated API client.
pub struct Client {
    agent: ureq::Agent,
    key: String,
    base: String,
}

impl Client {
    pub fn new(key: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        Self {
            agent,
            key: key.into(),
            base: BASE_URL.to_string(),
        }
    }

    /// Reads the key from the `CFBD_API_KEY` environment variable, falling
    /// back to `CFBD_API_KEY` or `API_KEY` in the `dotenv` file.
    pub fn from_env(dotenv: &Path) -> Result<Self> {
        let key = std::env::var("CFBD_API_KEY")
            .ok()
            .or_else(|| {
                let text = fs::read_to_string(dotenv).ok()?;
                dotenv_value(&text, "CFBD_API_KEY").or_else(|| dotenv_value(&text, "API_KEY"))
            })
            .with_context(|| {
                format!(
                    "no API key: set CFBD_API_KEY or put it in {} \
                     (free key at https://collegefootballdata.com/key)",
                    dotenv.display()
                )
            })?;
        if key.is_empty() {
            bail!("CFBD API key is empty");
        }
        Ok(Self::new(key))
    }

    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<String> {
        let url = format!("{}{}", self.base, path);
        let mut req = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.key))
            .header("Accept", "application/json");
        for (k, v) in query {
            req = req.query(*k, v);
        }
        let mut resp = req
            .call()
            .with_context(|| format!("GET {path} {query:?}"))?;
        resp.body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_string()
            .with_context(|| format!("reading body of {path}"))
    }

    /// Fetches one dataset and returns the raw JSON after checking it is an
    /// array, so an error object is never cached as data.
    pub fn fetch(&self, year: i32, ds: Dataset) -> Result<String> {
        let body = self.get(ds.path(), &ds.query(year))?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("{} {year}: response is not JSON", ds.path()))?;
        if !value.is_array() {
            bail!(
                "{} {year}: expected a JSON array, got {}",
                ds.path(),
                truncate(&body)
            );
        }
        Ok(body)
    }

    /// Fetches each requested dataset whose cache entry `refresh` deems stale.
    /// Returns the datasets actually fetched.
    pub fn sync(
        &self,
        cache: &Cache,
        year: i32,
        datasets: &[Dataset],
        refresh: Refresh,
    ) -> Result<Vec<Dataset>> {
        let mut fetched = Vec::new();
        for &ds in datasets {
            let stale = match (refresh, cache.age(year, ds)) {
                (_, None) | (Refresh::All, _) => true,
                (Refresh::Missing, Some(_)) => false,
                (Refresh::OlderThan(max), Some(age)) => ds.changes_in_season() && age > max,
            };
            if !stale {
                continue;
            }
            let body = self.fetch(year, ds)?;
            cache.store(year, ds, &body)?;
            fetched.push(ds);
        }
        Ok(fetched)
    }

    pub fn info(&self) -> Result<UserInfo> {
        let body = self.get("/info", &[])?;
        serde_json::from_str(&body).context("parsing /info")
    }
}

/// Value of `name` in `.env`-style text (`NAME=value`, optional `export`,
/// optional matching quotes, `#` comment lines).
fn dotenv_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (k, v) = line.split_once('=')?;
        if line.starts_with('#') || k.trim() != name {
            return None;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
            .unwrap_or(v);
        Some(v.to_string())
    })
}

fn truncate(s: &str) -> &str {
    match s.char_indices().nth(200) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::dotenv_value;

    #[test]
    fn dotenv_parsing() {
        let text = "# API_KEY=commented\nexport OTHER=1\nAPI_KEY = \"abc\"\nCFBD_API_KEY='x y'\n";
        assert_eq!(dotenv_value(text, "API_KEY").as_deref(), Some("abc"));
        assert_eq!(dotenv_value(text, "CFBD_API_KEY").as_deref(), Some("x y"));
        assert_eq!(dotenv_value(text, "OTHER").as_deref(), Some("1"));
        assert_eq!(dotenv_value(text, "MISSING"), None);
    }
}
