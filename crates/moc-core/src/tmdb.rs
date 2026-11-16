use serde_json::{Value, json};

pub fn image(base: &str, size: &str, path: &Value) -> Value {
    path.as_str()
        .filter(|path| path.starts_with('/'))
        .map(|path| json!(format!("{base}/{size}{path}")))
        .unwrap_or(Value::Null)
}

pub fn summary(base: &str, kind: &str, item: &Value, genres: &[(u64, String)]) -> Value {
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
                .find(|(genre, _)| Some(*genre) == id.as_u64())
                .map(|(_, name)| name.as_str())
        })
        .collect();
    let year = date.get(..4).unwrap_or("");
    json!({"id": format!("tmdb:{}", item["id"]), "type": kind,
        "name": item[if kind == "movie" { "title" } else { "name" }], "poster": image(base, "w342", &item["poster_path"]),
        "background": image(base, "w1280", &item["backdrop_path"]), "description": item["overview"],
        "year": year, "releaseInfo": year, "released": if date.len() == 10 { Some(format!("{date}T00:00:00Z")) } else { None }, "genres": names, "genre": names,
        "imdbRating": item["vote_average"].as_f64().filter(|v| *v > 0.0).map(|v| format!("{v:.1}"))})
}

pub fn runtime(minutes: Option<u64>) -> Value {
    match minutes.filter(|minutes| *minutes > 0) {
        Some(minutes) if minutes >= 60 => json!(format!("{}h{}min", minutes / 60, minutes % 60)),
        Some(minutes) => json!(format!("{minutes}min")),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summary_preserves_release_date_for_upcoming_cards() {
        let data = summary("https://images.test", "movie", &json!({"id":1,"title":"Test","release_date":"2027-02-14"}), &[]);
        assert_eq!(data["released"], "2027-02-14T00:00:00Z");
        let unknown = summary("https://images.test", "movie", &json!({"id":2,"title":"Test","release_date":""}), &[]);
        assert!(unknown["released"].is_null());
    }
}
