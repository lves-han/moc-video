fn halfwidth(c: char) -> char {
    match c {
        '\u{3000}' => ' ',
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        _ => c,
    }
}

fn separator(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || "、。，！？：；“”‘’《》〈〉【】（）「」『』·・—–…～".contains(c)
}

/// Full-width folded, punctuation removed, lowercase; ASCII words keep one space between them,
/// spaces next to CJK characters are dropped.
pub fn normalize(text: &str) -> String {
    let words: Vec<String> = text
        .chars()
        .map(halfwidth)
        .collect::<String>()
        .split(separator)
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut out = String::new();
    for word in words {
        let joins = out
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && word.starts_with(|c: char| c.is_ascii_alphanumeric());
        if joins {
            out.push(' ');
        }
        out.push_str(&word);
    }
    out
}

pub fn related(query: &str, title: &str) -> bool {
    let (q, t) = (normalize(query), normalize(title));
    !q.is_empty() && !t.is_empty() && (t.contains(&q) || t.chars().count() >= 2 && q.contains(&t))
}

/// Coordinates use the existing App's explicit season/episode label forms.
pub fn episode_coordinates(label: &str) -> Option<(Option<u32>, u32)> {
    let text = label.trim();
    let positive = |s: &str| s.parse::<u32>().ok().filter(|n| *n > 0);
    let lower = text.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix('s') {
        if let Some((season, number)) = rest.split_once('e') {
            return Some((Some(positive(season)?), positive(number)?));
        }
    }
    if let Some(rest) = text.strip_prefix('第') {
        if let Some((season, episode)) = rest.split_once('季') {
            return Some((
                Some(positive(season)?),
                positive(episode.strip_prefix('第')?.strip_suffix('集')?)?,
            ));
        }
        if let Some(number) = rest.strip_suffix('集') {
            return Some((None, positive(number)?));
        }
    }
    for prefix in ["episode", "ep", "e"] {
        if let Some(number) = lower.strip_prefix(prefix) {
            return Some((None, positive(number.trim())?));
        }
    }
    None
}

pub fn corresponding_episode<'a>(
    kind: &str,
    target: Option<(Option<u32>, u32)>,
    seasons: &[u32],
    episodes: &'a [serde_json::Value],
) -> Option<&'a serde_json::Value> {
    if kind == "movie" {
        return (episodes.len() == 1).then(|| &episodes[0]);
    }
    let (season, number) = target?;
    let mut unique_seasons = seasons.to_vec();
    unique_seasons.sort_unstable();
    unique_seasons.dedup();
    if season.is_none() && unique_seasons.len() > 1 {
        return None;
    }
    let inferred = (unique_seasons.len() == 1).then(|| unique_seasons[0]);
    let desired = season.or(inferred);
    let mut seen = std::collections::HashSet::new();
    let mut result = None;
    for episode in episodes {
        let (actual_season, actual_number) = episode_coordinates(episode["label"].as_str()?)?;
        if actual_season.is_none() && unique_seasons.len() > 1 {
            return None;
        }
        let key = (actual_season.or(inferred), actual_number);
        if !seen.insert(key) {
            return None;
        }
        if key == (desired, number) {
            result = Some(episode);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn episode_rules_reject_ambiguous_and_conflicting_seasons() {
        assert_eq!(episode_coordinates("S02E03"), Some((Some(2), 3)));
        assert_eq!(episode_coordinates("第2季第3集"), Some((Some(2), 3)));
        assert_eq!(episode_coordinates("Episode 3"), Some((None, 3)));
        let episodes = vec![json!({"label":"Episode 3"}), json!({"label":"Episode 4"})];
        assert!(corresponding_episode("series", Some((Some(1), 3)), &[1], &episodes).is_some());
        assert!(corresponding_episode("series", Some((Some(1), 3)), &[1, 2], &episodes).is_none());
        assert!(
            corresponding_episode(
                "series",
                Some((Some(1), 3)),
                &[1],
                &[episodes[0].clone(), episodes[0].clone()]
            )
            .is_none()
        );
        assert!(corresponding_episode("movie", None, &[], &episodes).is_none());
    }
}
