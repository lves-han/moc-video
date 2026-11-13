use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde_json::{Value, json};
use url::Url;

pub struct Tmdb {
    base: Url,
    key: String,
}

impl Tmdb {
    pub fn revision(&self) -> String {
        use sha2::{Digest, Sha256};
        format!(
            "{:x}",
            Sha256::digest(format!("{}\0{}", self.base, self.key))
        )
    }

    pub fn is_configured(&self) -> bool {
        !self.key.trim().is_empty()
    }

    pub fn from_env() -> Result<Self, &'static str> {
        let base = std::env::var("MOC_TMDB_API_URL")
            .unwrap_or_else(|_| "https://api.themoviedb.org/3".into());
        let base = Url::parse(&format!("{}/", base.trim_end_matches('/')))
            .map_err(|_| "invalid TMDB endpoint")?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err("invalid TMDB endpoint");
        }
        Ok(Self {
            base,
            key: std::env::var("MOC_TMDB_KEY").unwrap_or_default(),
        })
    }

    async fn api(&self, path: &str, params: &[(&str, String)]) -> Result<Value, &'static str> {
        if self.key.trim().is_empty() {
            return Err("TMDB API key is missing");
        }
        let mut url = self
            .base
            .join(path.trim_start_matches('/'))
            .map_err(|_| "invalid TMDB path")?;
        let mut headers = HeaderMap::new();
        if self.key.starts_with("eyJ") && self.key.contains('.') {
            headers.insert(
                AUTHORIZATION,
                format!("Bearer {}", self.key)
                    .parse()
                    .map_err(|_| "invalid TMDB key")?,
            );
        } else {
            url.query_pairs_mut().append_pair("api_key", &self.key);
        }
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(k, v)| (*k, v.as_str())));
        super::http::json(url, headers)
            .await
            .map_err(|error| match error {
                super::http::Error::Unauthorized => "TMDB API key was rejected",
                super::http::Error::Failed => "TMDB upstream request failed",
            })
    }

    async fn genres(&self, kind: &str, lang: &str) -> Result<Vec<(u64, String)>, &'static str> {
        let value = self
            .api(
                &format!("/genre/{}/list", media_type(kind)?),
                &[("language", lang.into())],
            )
            .await?;
        Ok(value["genres"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| Some((g["id"].as_u64()?, g["name"].as_str()?.to_owned())))
            .collect())
    }

    pub async fn manifest(&self, lang: &str) -> Result<Value, &'static str> {
        let languages = self.api("/configuration/languages", &[]).await?;
        let languages: Vec<String> = languages
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["english_name"].as_str())
            .filter(|v| !v.is_empty() && *v != "No Language")
            .map(str::to_owned)
            .collect();
        let countries = self.api("/configuration/countries", &[]).await?;
        let mut regions: Vec<String> = countries
            .as_array()
            .ok_or("invalid TMDB countries")?
            .iter()
            .filter_map(|item| item["iso_3166_1"].as_str())
            .filter(|code| code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase()))
            .map(str::to_owned)
            .collect();
        regions.sort();
        regions.dedup();
        let preferred = [
            "CN", "HK", "TW", "US", "JP", "KR", "GB", "FR", "DE", "CA", "AU", "IN", "TH", "SG",
        ];
        regions.sort_by_key(|code| {
            (
                preferred
                    .iter()
                    .position(|item| *item == code)
                    .unwrap_or(preferred.len()),
                code.clone(),
            )
        });
        let mut catalogs = Vec::new();
        for kind in ["movie", "series"] {
            let genres: Vec<String> = self
                .genres(kind, lang)
                .await?
                .into_iter()
                .map(|(_, name)| name)
                .collect();
            for (id, name) in [
                ("tmdb.top", "Popular"),
                ("tmdb.year", "Year"),
                ("tmdb.language", "Language"),
                ("tmdb.trending", "Trending"),
                ("tmdb.latest", "Latest Releases"),
            ] {
                let options = match id {
                    "tmdb.language" => languages.clone(),
                    "tmdb.trending" => vec!["Day".into(), "Week".into()],
                    "tmdb.year" => (0..21)
                        .map(|n| (time::OffsetDateTime::now_utc().year() - n).to_string())
                        .collect(),
                    _ => genres.clone(),
                };
                let mut extras = vec![
                    json!({"name":"genre","options":options}),
                    json!({"name":"skip"}),
                ];
                if matches!(
                    id,
                    "tmdb.top" | "tmdb.latest" | "tmdb.year" | "tmdb.language"
                ) {
                    extras.push(json!({"name":"region","options":regions}));
                }
                catalogs.push(json!({"type":kind,"id":id,"name":name,"extra":extras}));
            }
            catalogs.push(json!({"type":kind,"id":"tmdb.search","name":"Search","extra":[{"name":"search","isRequired":true}]}));
            let additions = if kind == "movie" {
                vec![
                    ("tmdb.now_playing", "Now Playing"),
                    ("tmdb.upcoming", "Upcoming"),
                    ("tmdb.top_rated", "Top Rated"),
                    ("tmdb.animation", "Animation"),
                    ("tmdb.documentary", "Documentary"),
                ]
            } else {
                vec![
                    ("tmdb.on_air", "On Air"),
                    ("tmdb.top_rated", "Top Rated"),
                    ("tmdb.animation", "Animation"),
                    ("tmdb.documentary", "Documentary"),
                    ("tmdb.variety", "Variety"),
                ]
            };
            for (id, name) in additions {
                catalogs.push(json!({"type":kind,"id":id,"name":name,"extra":[{"name":"skip"}]}));
            }
        }
        Ok(
            json!({"id":"tmdb-addon","version":"1.0.0","name":"TMDB API","description":"TMDB official API","resources":["catalog","meta"],"types":["movie","series"],"idPrefixes":["tmdb:"],"catalogs":catalogs}),
        )
    }

    pub async fn catalog(
        &self,
        lang: &str,
        kind: &str,
        id: &str,
        extra: &str,
    ) -> Result<Value, &'static str> {
        let media = media_type(kind)?;
        let extras: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(extra.as_bytes())
                .into_owned()
                .collect();
        let page = extras
            .get("skip")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            / 20
            + 1;
        let genres = self.genres(kind, lang).await?;
        let mut query = vec![("language", lang.to_owned()), ("page", page.to_string())];
        let path = match id {
            "tmdb.search" => {
                query.push(("query", extras.get("search").cloned().unwrap_or_default()));
                format!("/search/{media}")
            }
            "tmdb.trending" => format!(
                "/trending/{media}/{}",
                if extras.get("genre").map(String::as_str) == Some("Week") {
                    "week"
                } else {
                    "day"
                }
            ),
            "tmdb.now_playing" if media == "movie" => "/movie/now_playing".to_owned(),
            "tmdb.upcoming" if media == "movie" => "/movie/upcoming".to_owned(),
            "tmdb.on_air" if media == "tv" => "/tv/on_the_air".to_owned(),
            "tmdb.top_rated" => format!("/{media}/top_rated"),
            "tmdb.animation" | "tmdb.documentary" | "tmdb.variety" => {
                if id == "tmdb.variety" && media != "tv" {
                    return Err("unknown TMDB catalog");
                }
                query.push((
                    "with_genres",
                    match id {
                        "tmdb.animation" => "16",
                        "tmdb.documentary" => "99",
                        _ => "10764|10767",
                    }
                    .to_owned(),
                ));
                query.push(("sort_by", "popularity.desc".to_owned()));
                format!("/discover/{media}")
            }
            "tmdb.top" | "tmdb.latest" | "tmdb.year" | "tmdb.language" => {
                query.push((
                    "sort_by",
                    if id == "tmdb.latest" {
                        if media == "movie" {
                            "primary_release_date.desc"
                        } else {
                            "first_air_date.desc"
                        }
                    } else {
                        "popularity.desc"
                    }
                    .to_owned(),
                ));
                if id == "tmdb.latest" {
                    query.push((
                        if media == "movie" {
                            "primary_release_date.lte"
                        } else {
                            "first_air_date.lte"
                        },
                        time::OffsetDateTime::now_utc().date().to_string(),
                    ));
                    query.push(("vote_count.gte", "10".into()));
                }
                if let Some(filter) = extras.get("genre") {
                    match id {
                        "tmdb.year" => query.push((
                            if media == "movie" {
                                "primary_release_year"
                            } else {
                                "first_air_date_year"
                            },
                            filter.clone(),
                        )),
                        "tmdb.language" => {
                            let languages = self.api("/configuration/languages", &[]).await?;
                            let code = languages
                                .as_array()
                                .into_iter()
                                .flatten()
                                .find(|v| v["english_name"] == *filter)
                                .and_then(|v| v["iso_639_1"].as_str())
                                .ok_or("unknown TMDB language")?;
                            query.push(("with_original_language", code.to_owned()));
                        }
                        _ => {
                            let english = self.genres(kind, "en-US").await?;
                            let genre = genres
                                .iter()
                                .chain(english.iter())
                                .find(|(_, name)| name == filter)
                                .ok_or("unknown TMDB genre")?;
                            query.push(("with_genres", genre.0.to_string()));
                        }
                    }
                }
                format!("/discover/{media}")
            }
            _ => return Err("unknown TMDB catalog"),
        };
        if let Some(category) = extras.get("category") {
            if !path.starts_with("/discover/") {
                return Err("invalid TMDB category");
            }
            let genre = match (category.as_str(), media) {
                ("animation", _) => "16",
                ("documentary", _) => "99",
                ("variety", "tv") => "10764|10767",
                _ => return Err("invalid TMDB category"),
            };
            if query.iter().any(|(key, _)| *key == "with_genres") {
                return Err("invalid TMDB category combination");
            }
            query.push(("with_genres", genre.to_owned()));
        }
        if let Some(region) = extras.get("region") {
            if !path.starts_with("/discover/")
                || region.len() != 2
                || !region.bytes().all(|b| b.is_ascii_uppercase())
            {
                return Err("invalid TMDB origin region");
            }
            query.push(("with_origin_country", region.clone()));
        }
        let value = self.api(&path, &query).await?;
        let metas: Vec<_> = value["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["id"].is_u64())
            .map(|item| moc_core::tmdb::summary("https://image.tmdb.org/t/p", kind, item, &genres))
            .collect();
        Ok(json!({"metas":metas}))
    }

    pub async fn meta(&self, lang: &str, kind: &str, id: &str) -> Result<Value, &'static str> {
        let media = media_type(kind)?;
        let number = id
            .strip_prefix("tmdb:")
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .ok_or("invalid TMDB id")?;
        let item = self
            .api(
                &format!("/{media}/{number}"),
                &[
                    ("language", lang.into()),
                    (
                        "append_to_response",
                        "credits,external_ids,alternative_titles".into(),
                    ),
                ],
            )
            .await?;
        if item["id"].as_u64().map(|n| n.to_string()).as_deref() != Some(number) {
            return Err("TMDB identity mismatch");
        }
        let imdb = item["external_ids"]["imdb_id"]
            .as_str()
            .filter(|id| id.starts_with("tt"));
        let image = |size: &str, path: &Value| {
            moc_core::tmdb::image("https://image.tmdb.org/t/p", size, path)
        };
        let start = item[if media == "movie" {
            "release_date"
        } else {
            "first_air_date"
        }]
        .as_str()
        .unwrap_or("");
        let genres: Vec<Value> = item["genres"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| v["name"].clone())
            .collect();
        let cast: Vec<&Value> = item["credits"]["cast"]
            .as_array()
            .into_iter()
            .flatten()
            .take(15)
            .collect();
        let country = if lang == "zh-CN" { "CN" } else { "US" };
        let aliases: Vec<_> = ["titles", "results"]
            .iter()
            .flat_map(|key| {
                item["alternative_titles"][*key]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .filter(|v| v["iso_3166_1"] == country)
            .filter_map(|v| v["title"].as_str())
            .take(3)
            .collect();
        let mut meta = json!({"id":id,"type":kind,"name":item[if media == "movie" {"title"} else {"name"}],
            "original_title":item[if media == "movie" {"original_title"} else {"original_name"}],"aliases":aliases,
            "poster":image("w500",&item["poster_path"]),"background":image("original",&item["backdrop_path"]),
            "description":item["overview"],"genres":genres,"genre":genres,"year":start.get(..4).unwrap_or(""),
            "releaseInfo":start.get(..4).unwrap_or(""),"imdb_id":imdb,
            "imdbRating":item["vote_average"].as_f64().filter(|v| *v>0.0).map(|v|format!("{v:.1}")),
            "country":item["production_countries"].as_array().into_iter().flatten().filter_map(|v| v["name"].as_str()).collect::<Vec<_>>().join(", "),
            "cast":cast.iter().map(|v| v["name"].clone()).collect::<Vec<_>>(),
            "app_extras":{"cast":cast.iter().map(|v|json!({"name":v["name"],"character":v["character"],"photo":image("w276_and_h350_face",&v["profile_path"])})).collect::<Vec<_>>()}});
        if media == "movie" {
            meta["runtime"] = moc_core::tmdb::runtime(item["runtime"].as_u64());
            meta["director"] = json!(
                item["credits"]["crew"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| v["job"] == "Director")
                    .map(|v| v["name"].clone())
                    .collect::<Vec<_>>()
            );
            meta["behaviorHints"] = json!({"defaultVideoId":imdb});
        } else {
            let end = item["last_air_date"]
                .as_str()
                .and_then(|d| d.get(..4))
                .filter(|_| item["status"] == "Ended");
            meta["releaseInfo"] = json!(match end {
                Some(end) => format!("{}-{end}", start.get(..4).unwrap_or("")),
                None => format!("{}-", start.get(..4).unwrap_or("")),
            });
            meta["runtime"] = moc_core::tmdb::runtime(item["episode_run_time"][0].as_u64());
            meta["director"] = json!(
                item["created_by"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|v| v["name"].clone())
                    .collect::<Vec<_>>()
            );
            let prefix = imdb
                .map(str::to_owned)
                .unwrap_or_else(|| format!("tmdb:{number}"));
            let mut videos = Vec::new();
            for season in item["seasons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s["season_number"].as_u64())
                .take(60)
            {
                let detail = self
                    .api(
                        &format!("/tv/{number}/season/{season}"),
                        &[("language", lang.into())],
                    )
                    .await?;
                for ep in detail["episodes"].as_array().into_iter().flatten() {
                    let Some(n) = ep["episode_number"].as_u64() else {
                        continue;
                    };
                    let released = ep["air_date"]
                        .as_str()
                        .filter(|d| d.len() == 10)
                        .map(|d| format!("{d}T00:00:00.000Z"));
                    videos.push(json!({"id":format!("{prefix}:{season}:{n}"),"name":ep["name"],"title":ep["name"],"season":season,"number":n,"episode":n,"overview":ep["overview"],"description":ep["overview"],"thumbnail":image("w500",&ep["still_path"]),"released":released,"runtime":moc_core::tmdb::runtime(ep["runtime"].as_u64())}));
                }
            }
            meta["videos"] = json!(videos);
        }
        Ok(json!({"meta":meta}))
    }
}

fn media_type(kind: &str) -> Result<&'static str, &'static str> {
    match kind {
        "movie" => Ok("movie"),
        "series" => Ok("tv"),
        _ => Err("unknown media type"),
    }
}
