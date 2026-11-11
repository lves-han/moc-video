//! TMDB official API mode. The built-in TMDB source registers a virtual addon
//! address on `HOST`; Stremio fetches to it are answered here by calling the TMDB
//! v3 API and returning the same catalog/meta shapes the TMDB Stremio addon uses.
//! Everything downstream (localization, filters, details, matching) is shared,
//! and content IDs (`tmdb:<id>`, series videos `<imdb>:<s>:<e>`) match the addon.
use super::maccms::{Http, Response};
use crate::{contracts::ApiError, requests::Job};
use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex};
use url::Url;

pub const HOST: &str = "tmdb-api.invalid";
pub const API_URL: &str = "https://api.themoviedb.org/3";
pub const IMAGE_URL: &str = "https://image.tmdb.org/t/p";
const BODY_LIMIT: usize = 8 * 1024 * 1024;
const SEASON_LIMIT: usize = 60;

#[derive(Clone, PartialEq)]
struct Credentials {
    key: String,
    api_url: String,
    image_url: String,
}

// The key lives only here and in the settings file, never in a source or resource URL.
static CREDENTIALS: Mutex<Option<Credentials>> = Mutex::new(None);
// Genre lists by type and language, reset when the credentials change.
type GenreCache = Option<HashMap<String, Vec<(u64, String)>>>;
static GENRES: Mutex<GenreCache> = Mutex::new(None);

fn credentials(settings: &Value) -> Option<Credentials> {
    let tmdb = &settings["tmdb"];
    (tmdb["mode"] == "api").then(|| Credentials {
        key: tmdb["api_key"].as_str().unwrap_or("").trim().to_owned(),
        api_url: tmdb["api_url"]
            .as_str()
            .unwrap_or(API_URL)
            .trim_end_matches('/')
            .to_owned(),
        image_url: tmdb["image_url"]
            .as_str()
            .unwrap_or(IMAGE_URL)
            .trim_end_matches('/')
            .to_owned(),
    })
}

pub fn configure(settings: &Value) {
    let next = credentials(settings);
    let mut current = CREDENTIALS.lock().expect("TMDB credentials");
    if *current != next {
        GENRES.lock().expect("TMDB genres").take();
    }
    *current = next;
}

/// Non-secret fingerprint so a changed key or endpoint re-synchronizes the source.
pub fn fingerprint(settings: &Value) -> Option<String> {
    credentials(settings).map(|c| {
        format!(
            "{:x}",
            Sha256::digest(format!("{}\0{}\0{}", c.key, c.api_url, c.image_url))
        )[..16]
            .to_owned()
    })
}

pub fn manifest_url() -> String {
    format!("https://{HOST}/en-US/manifest.json")
}

fn bad_key() -> ApiError {
    ApiError::new("UNAUTHORIZED", "TMDB API key was rejected")
}

fn api(
    http: &Http,
    creds: &Credentials,
    job: &Job,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, ApiError> {
    if creds.key.is_empty() {
        return Err(ApiError::new("UNAUTHORIZED", "TMDB API key is missing"));
    }
    let mut url = Url::parse(&format!("{}{path}", creds.api_url))
        .map_err(|_| ApiError::bad("TMDB API address is invalid"))?;
    let mut headers = HeaderMap::new();
    // v4 read access tokens are JWTs and go in a header; v3 keys are a query parameter.
    if creds.key.starts_with("eyJ") && creds.key.contains('.') {
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", creds.key)
                .parse()
                .map_err(|_| bad_key())?,
        );
    } else {
        url.query_pairs_mut().append_pair("api_key", &creds.key);
    }
    url.query_pairs_mut()
        .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
    let response = http
        .get(url, headers, job, BODY_LIMIT, false, false)
        .map_err(|error| {
            match error
                .details
                .as_ref()
                .and_then(|d| d["http_status"].as_u64())
            {
                Some(401) => bad_key(),
                Some(404) => ApiError::new("UPSTREAM_FAILED", "TMDB API has no such title"),
                // Messages never carry the request URL, which holds a v3 key.
                _ => ApiError {
                    details: None,
                    ..error
                },
            }
        })?;
    serde_json::from_slice(&response.body)
        .map_err(|_| ApiError::new("UPSTREAM_FAILED", "TMDB API response is not JSON"))
}

fn tmdb_type(kind: &str) -> Result<&'static str, ApiError> {
    match kind {
        "movie" => Ok("movie"),
        "series" => Ok("tv"),
        _ => Err(ApiError::bad("TMDB API supports movie and series")),
    }
}

fn genres(
    http: &Http,
    creds: &Credentials,
    job: &Job,
    kind: &str,
    lang: &str,
) -> Result<Vec<(u64, String)>, ApiError> {
    let key = format!("{kind}\0{lang}");
    if let Some(list) = GENRES
        .lock()
        .expect("TMDB genres")
        .as_ref()
        .and_then(|m| m.get(&key))
    {
        return Ok(list.clone());
    }
    let value = api(
        http,
        creds,
        job,
        &format!("/genre/{}/list", tmdb_type(kind)?),
        &[("language", lang.into())],
    )?;
    let list: Vec<_> = value["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| Some((g["id"].as_u64()?, g["name"].as_str()?.to_owned())))
        .collect();
    GENRES
        .lock()
        .expect("TMDB genres")
        .get_or_insert_with(HashMap::new)
        .insert(key, list.clone());
    Ok(list)
}

fn languages(
    http: &Http,
    creds: &Credentials,
    job: &Job,
) -> Result<Vec<(String, String)>, ApiError> {
    let value = api(http, creds, job, "/configuration/languages", &[])?;
    let mut list: Vec<(String, String)> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| {
            Some((
                l["iso_639_1"].as_str()?.to_owned(),
                l["english_name"].as_str()?.to_owned(),
            ))
        })
        .filter(|(_, name)| !name.is_empty() && name != "No Language")
        .collect();
    list.sort_by(|a, b| (a.1 != "English", &a.1).cmp(&(b.1 != "English", &b.1)));
    Ok(list)
}

fn year() -> i32 {
    time::OffsetDateTime::now_utc().year()
}

fn manifest(http: &Http, creds: &Credentials, job: &Job, lang: &str) -> Result<Value, ApiError> {
    let languages: Vec<String> = languages(http, creds, job)?
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    let years: Vec<String> = (0..21).map(|n| (year() - n).to_string()).collect();
    let mut catalogs = Vec::new();
    for kind in ["movie", "series"] {
        let names: Vec<String> = genres(http, creds, job, kind, lang)?
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        let catalog = |id: &str, name: &str, options: &[String]| {
            json!({"type": kind, "id": id, "name": name,
            "extra": [{"name": "genre", "isRequired": false, "options": options}, {"name": "skip"}]})
        };
        catalogs.extend([
            catalog("tmdb.top", "Popular", &names),
            catalog("tmdb.year", "Year", &years),
            catalog("tmdb.language", "Language", &languages),
            catalog("tmdb.trending", "Trending", &["Day".into(), "Week".into()]),
            catalog("tmdb.latest", "Latest Releases", &names),
            json!({"type": kind, "id": "tmdb.search", "name": "Search", "extra": [{"name": "search", "isRequired": true}]}),
        ]);
    }
    Ok(
        json!({"id": "tmdb-addon", "version": "1.0.0", "name": "TMDB API", "description": "TMDB official API",
        "resources": ["catalog", "meta"], "types": ["movie", "series"], "idPrefixes": ["tmdb:"], "catalogs": catalogs}),
    )
}

fn image(creds: &Credentials, size: &str, path: &Value) -> Value {
    path.as_str()
        .filter(|p| p.starts_with('/'))
        .map(|p| json!(format!("{}/{size}{p}", creds.image_url)))
        .unwrap_or(Value::Null)
}

fn summary(creds: &Credentials, kind: &str, item: &Value, genres: &[(u64, String)]) -> Value {
    let date = item[if kind == "movie" {
        "release_date"
    } else {
        "first_air_date"
    }]
    .as_str()
    .unwrap_or("");
    let names: Vec<&str> = item["genre_ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| {
            genres
                .iter()
                .find(|(g, _)| Some(*g) == id.as_u64())
                .map(|(_, n)| n.as_str())
        })
        .collect();
    let year = date.get(..4).unwrap_or("");
    json!({"id": format!("tmdb:{}", item["id"]), "type": kind,
        "name": item[if kind == "movie" { "title" } else { "name" }], "poster": image(creds, "w342", &item["poster_path"]),
        "background": image(creds, "w1280", &item["backdrop_path"]), "description": item["overview"],
        "year": year, "releaseInfo": year, "genres": names, "genre": names,
        "imdbRating": item["vote_average"].as_f64().filter(|v| *v > 0.0).map(|v| format!("{v:.1}"))})
}

fn catalog(
    http: &Http,
    creds: &Credentials,
    job: &Job,
    lang: &str,
    kind: &str,
    id: &str,
    extra: &str,
) -> Result<Value, ApiError> {
    let t = tmdb_type(kind)?;
    let extras: HashMap<String, String> = url::form_urlencoded::parse(extra.as_bytes())
        .into_owned()
        .collect();
    let page = extras
        .get("skip")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
        / 20
        + 1;
    let genre = extras.get("genre").map(String::as_str);
    let localized = genres(http, creds, job, kind, lang)?;
    let genre_id = |name: &str| -> Result<Option<String>, ApiError> {
        let english = genres(http, creds, job, kind, "en-US")?;
        Ok(localized
            .iter()
            .chain(english.iter())
            .find(|(_, n)| n == name)
            .map(|(id, _)| id.to_string()))
    };
    let date_field = if t == "movie" {
        "primary_release_date"
    } else {
        "first_air_date"
    };
    let mut query = vec![("language", lang.to_owned()), ("page", page.to_string())];
    let path = match id {
        "tmdb.search" => {
            query.push(("query", extras.get("search").cloned().unwrap_or_default()));
            format!("/search/{t}")
        }
        "tmdb.trending" => format!(
            "/trending/{t}/{}",
            if genre == Some("Week") { "week" } else { "day" }
        ),
        "tmdb.top" | "tmdb.latest" | "tmdb.year" | "tmdb.language" => {
            match id {
                "tmdb.latest" => {
                    query.push(("sort_by", format!("{date_field}.desc")));
                    let today = time::OffsetDateTime::now_utc().date();
                    query.push((
                        if t == "movie" {
                            "primary_release_date.lte"
                        } else {
                            "first_air_date.lte"
                        },
                        today.to_string(),
                    ));
                    query.push(("vote_count.gte", "10".into()));
                }
                _ => query.push(("sort_by", "popularity.desc".into())),
            }
            if let Some(genre) = genre {
                match id {
                    "tmdb.year" => query.push((
                        if t == "movie" {
                            "primary_release_year"
                        } else {
                            "first_air_date_year"
                        },
                        genre.to_owned(),
                    )),
                    "tmdb.language" => {
                        let code = languages(http, creds, job)?
                            .into_iter()
                            .find(|(_, n)| n == genre)
                            .map(|(c, _)| c);
                        query.push((
                            "with_original_language",
                            code.ok_or_else(|| ApiError::bad("Unknown TMDB language"))?,
                        ));
                    }
                    _ => query.push((
                        "with_genres",
                        genre_id(genre)?.ok_or_else(|| ApiError::bad("Unknown TMDB genre"))?,
                    )),
                }
            }
            format!("/discover/{t}")
        }
        _ => return Err(ApiError::bad("Unknown TMDB catalog")),
    };
    let value = api(http, creds, job, &path, &query)?;
    let metas: Vec<Value> = value["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["id"].is_u64())
        .map(|item| summary(creds, kind, item, &localized))
        .collect();
    Ok(json!({"metas": metas}))
}

fn runtime(minutes: Option<u64>) -> Value {
    match minutes.filter(|m| *m > 0) {
        Some(m) if m >= 60 => json!(format!("{}h{}min", m / 60, m % 60)),
        Some(m) => json!(format!("{m}min")),
        None => Value::Null,
    }
}

fn meta(
    http: &Http,
    creds: &Credentials,
    job: &Job,
    lang: &str,
    kind: &str,
    id: &str,
) -> Result<Value, ApiError> {
    let t = tmdb_type(kind)?;
    let number = id
        .strip_prefix("tmdb:")
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| ApiError::bad("TMDB API meta requires a tmdb: ID"))?;
    let item = api(
        http,
        creds,
        job,
        &format!("/{t}/{number}"),
        &[
            ("language", lang.into()),
            ("append_to_response", "credits,external_ids".into()),
        ],
    )?;
    let genres: Vec<Value> = item["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| g["name"].clone())
        .collect();
    let cast: Vec<&Value> = item["credits"]["cast"]
        .as_array()
        .into_iter()
        .flatten()
        .take(15)
        .collect();
    let imdb = item["external_ids"]["imdb_id"]
        .as_str()
        .filter(|s| s.starts_with("tt"))
        .map(str::to_owned);
    let start = item[if t == "movie" {
        "release_date"
    } else {
        "first_air_date"
    }]
    .as_str()
    .unwrap_or("");
    let mut meta = json!({"id": format!("tmdb:{number}"), "type": kind, "name": item[if t == "movie" { "title" } else { "name" }],
        "poster": image(creds, "w500", &item["poster_path"]), "background": image(creds, "original", &item["backdrop_path"]),
        "description": item["overview"], "genres": genres, "genre": genres, "year": start.get(..4).unwrap_or(""),
        "releaseInfo": start.get(..4).unwrap_or(""), "imdb_id": imdb,
        "imdbRating": item["vote_average"].as_f64().filter(|v| *v > 0.0).map(|v| format!("{v:.1}")),
        "country": item["production_countries"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str()).collect::<Vec<_>>().join(", "),
        "cast": cast.iter().map(|c| c["name"].clone()).collect::<Vec<_>>(),
        "app_extras": {"cast": cast.iter().map(|c| json!({"name": c["name"], "character": c["character"],
            "photo": image(creds, "w276_and_h350_face", &c["profile_path"])})).collect::<Vec<_>>()}});
    if t == "movie" {
        meta["runtime"] = runtime(item["runtime"].as_u64());
        meta["director"] = json!(
            item["credits"]["crew"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["job"] == "Director")
                .map(|c| c["name"].clone())
                .collect::<Vec<_>>()
        );
        meta["behaviorHints"] = json!({"defaultVideoId": imdb});
        return Ok(json!({"meta": meta}));
    }
    let end = item["last_air_date"]
        .as_str()
        .and_then(|d| d.get(..4))
        .filter(|_| item["status"] == "Ended");
    meta["releaseInfo"] = json!(match end {
        Some(end) => format!("{}-{end}", start.get(..4).unwrap_or("")),
        None => format!("{}-", start.get(..4).unwrap_or("")),
    });
    meta["runtime"] = runtime(item["episode_run_time"][0].as_u64());
    meta["director"] = json!(
        item["created_by"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| c["name"].clone())
            .collect::<Vec<_>>()
    );
    // Episode IDs follow the addon: IMDb-based when known, so history matches both modes.
    let prefix = imdb.unwrap_or_else(|| format!("tmdb:{number}"));
    let mut videos = Vec::new();
    for season in item["seasons"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["season_number"].as_u64())
        .take(SEASON_LIMIT)
    {
        let detail = api(
            http,
            creds,
            job,
            &format!("/tv/{number}/season/{season}"),
            &[("language", lang.into())],
        )?;
        for episode in detail["episodes"].as_array().into_iter().flatten() {
            let Some(n) = episode["episode_number"].as_u64() else {
                continue;
            };
            let aired = episode["air_date"]
                .as_str()
                .filter(|d| d.len() == 10)
                .map(|d| format!("{d}T00:00:00.000Z"));
            videos.push(json!({"id": format!("{prefix}:{season}:{n}"), "name": episode["name"], "title": episode["name"],
                "season": season, "number": n, "episode": n, "overview": episode["overview"], "description": episode["overview"],
                "thumbnail": image(creds, "w500", &episode["still_path"]), "released": aired,
                "runtime": runtime(episode["runtime"].as_u64())}));
        }
    }
    meta["videos"] = json!(videos);
    Ok(json!({"meta": meta}))
}

// Path segments are percent-encoded once by the Stremio URL builder.
fn decode_segment(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Answers a Stremio addon request addressed to the virtual TMDB API host.
pub fn respond(http: &Http, url: &Url, job: &Job) -> Result<Response, ApiError> {
    let creds = CREDENTIALS
        .lock()
        .expect("TMDB credentials")
        .clone()
        .ok_or_else(|| ApiError::new("CAPABILITY_UNAVAILABLE", "TMDB API mode is not enabled"))?;
    let segments: Vec<String> = url
        .path_segments()
        .into_iter()
        .flatten()
        .map(decode_segment)
        .collect();
    let strip = |s: &str| s.strip_suffix(".json").map(str::to_owned);
    let body = match segments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [lang, "manifest.json"] => manifest(http, &creds, job, lang)?,
        [lang, "catalog", kind, id] => catalog(
            http,
            &creds,
            job,
            lang,
            kind,
            &strip(id).ok_or_else(|| ApiError::bad("Invalid catalog path"))?,
            "",
        )?,
        [lang, "catalog", kind, id, extra] => catalog(
            http,
            &creds,
            job,
            lang,
            kind,
            id,
            &strip(extra).ok_or_else(|| ApiError::bad("Invalid catalog path"))?,
        )?,
        [lang, "meta", kind, id] => meta(
            http,
            &creds,
            job,
            lang,
            kind,
            &strip(id).ok_or_else(|| ApiError::bad("Invalid meta path"))?,
        )?,
        _ => return Err(ApiError::bad("Unknown TMDB API addon resource")),
    };
    Ok(Response {
        url: url.clone(),
        headers: HeaderMap::new(),
        body: body.to_string().into_bytes(),
    })
}
