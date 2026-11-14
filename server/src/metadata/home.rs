use super::{
    cache::{self, Cache},
    tmdb::Tmdb,
};
use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;

const MOVIE: &[&str] = &[
    "tmdb.top",
    "tmdb.trending",
    "tmdb.now_playing",
    "tmdb.upcoming",
    "tmdb.top_rated",
    "tmdb.animation",
    "tmdb.documentary",
];
const SERIES: &[&str] = &[
    "tmdb.top",
    "tmdb.trending",
    "tmdb.on_air",
    "tmdb.top_rated",
    "tmdb.animation",
    "tmdb.documentary",
    "tmdb.variety",
];

#[derive(Default)]
pub struct Home {
    pub built_at: Option<i64>,
    pub payload: Option<Value>,
    pub error: Option<String>,
}

pub fn is_home(kind: &str, id: &str) -> bool {
    match kind {
        "movie" => MOVIE.contains(&id),
        "series" => SERIES.contains(&id),
        _ => false,
    }
}

fn last_daily(now: i64, zone: Tz) -> Option<i64> {
    let local = DateTime::<Utc>::from_timestamp(now, 0)?.with_timezone(&zone);
    let mut date = local.date_naive();
    if local.time() < NaiveTime::from_hms_opt(4, 0, 0)? {
        date = date.checked_sub_signed(Duration::days(1))?;
    }
    let naive = date.and_hms_opt(4, 0, 0)?;
    zone.from_local_datetime(&naive)
        .earliest()
        .map(|v| v.timestamp())
}

pub fn initialize(cache: &Cache) -> Result<Home, rusqlite::Error> {
    Ok(match cache.snapshot()? {
        Some((built_at, payload)) => Home {
            built_at: Some(built_at),
            payload: Some(payload),
            error: None,
        },
        None => Home::default(),
    })
}

pub fn start(cache: Cache, tmdb: Arc<Tmdb>, home: Arc<RwLock<Home>>, zone: Tz, poll: u64) {
    tokio::spawn(async move {
        let mut pruned_at = i64::MIN;
        loop {
            let now = cache::now();
            if now.saturating_sub(pruned_at) >= 3600 {
                pruned_at = now;
                if cache.prune_async(now).await.is_err() {
                    eprintln!("Metadata cache cleanup failed; retrying within an hour");
                }
            }
            let due = last_daily(cache::now(), zone)
                .map(|cutoff| {
                    home.try_read()
                        .map(|state| state.built_at.is_none_or(|built| built < cutoff))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            let previous = home.read().await.payload.clone();
            if due {
                match build(&tmdb, previous.as_ref()).await {
                    Ok(snapshot) => {
                        let at = cache::now();
                        match cache.save_snapshot_async(at, &snapshot).await {
                            Ok(()) => {
                                let mut state = home.write().await;
                                state.built_at = Some(at);
                                state.payload = Some(snapshot);
                                state.error = None;
                            }
                            Err(_) => {
                                home.write().await.error =
                                    Some("saving home snapshot failed".into());
                            }
                        }
                    }
                    Err(error) => {
                        home.write().await.error = Some(error.into());
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(poll)).await;
        }
    });
}

async fn build(tmdb: &Tmdb, previous: Option<&Value>) -> Result<Value, &'static str> {
    let mut all = serde_json::Map::new();
    let mut any_ok = false;
    let mut last_err = None;
    for lang in ["zh-CN", "en-US"] {
        let mut catalogs = serde_json::Map::new();
        let requests: Vec<_> = [("movie", MOVIE), ("series", SERIES)].into_iter()
            .flat_map(|(kind, ids)| ids.iter().flat_map(move |id| [0, 20].into_iter().map(move |page| (kind, *id, page))))
            .collect();
        for chunk in requests.chunks(4) {
            let mut jobs = tokio::task::JoinSet::new();
            for &(kind, id, page) in chunk {
                let tmdb = tmdb.clone();
                jobs.spawn(async move { (kind, id, page, tmdb.catalog(lang, kind, id, &format!("skip={page}")).await) });
            }
            while let Some(result) = jobs.join_next().await {
                let Ok((kind, id, page, value)) = result else {
                    last_err = Some("home snapshot request failed");
                    continue;
                };
                let slot = format!("{kind}/{id}/{page}");
                match value {
                    Ok(v) => {
                        any_ok = true;
                        catalogs.insert(slot, v);
                    }
                    Err(err) => {
                        last_err = Some(err);
                        if let Some(prev) = previous.and_then(|p| p.get(lang)).and_then(|l| l.get(&slot)) {
                            catalogs.insert(slot, prev.clone());
                        }
                    }
                }
            }
        }
        all.insert(lang.into(), json!(catalogs));
    }
    if !any_ok {
        return Err(last_err.unwrap_or("home snapshot request failed"));
    }
    Ok(Value::Object(all))
}

pub fn find(payload: &Value, lang: &str, kind: &str, id: &str, extra: &str) -> Option<Value> {
    let extras: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(extra.as_bytes())
            .into_owned()
            .collect();
    if extras.keys().any(|key| key != "skip") {
        return None;
    }
    let skip = extras
        .get("skip")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    if !matches!(skip, 0 | 20) {
        return None;
    }
    payload
        .get(lang)?
        .get(format!("{kind}/{id}/{skip}"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_cutoff_uses_configured_timezone() {
        let zone = chrono_tz::Asia::Shanghai;
        let now = DateTime::parse_from_rfc3339("2026-11-12T03:59:00+08:00")
            .unwrap()
            .timestamp();
        assert_eq!(
            last_daily(now, zone),
            Some(
                DateTime::parse_from_rfc3339("2026-11-11T04:00:00+08:00")
                    .unwrap()
                    .timestamp()
            )
        );
        assert_eq!(
            last_daily(now + 120, zone),
            Some(
                DateTime::parse_from_rfc3339("2026-11-12T04:00:00+08:00")
                    .unwrap()
                    .timestamp()
            )
        );
    }
}
