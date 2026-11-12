//! Approximate title search for MacCMS sources, whose `wd` query is a plain
//! substring match: normalized and progressively relaxed queries, then ranking
//! by closeness to what the user typed.
use serde_json::Value;

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

const NUMERALS: &str = "0123456789一二三四五六七八九十百零";
const ROMAN: [&str; 9] = ["ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];

/// Season/part/episode markers and trailing sequel numbers removed from a normalized title.
fn without_sequel(text: &str) -> String {
    let mut text = text.to_owned();
    // 第二季 / 第3部 / 第10集
    if let Some(start) = text.rfind('第') {
        let tail: Vec<char> = text[start + '第'.len_utf8()..].chars().collect();
        if tail.len() >= 2
            && tail[..tail.len() - 1].iter().all(|c| NUMERALS.contains(*c))
            && "季部集".contains(tail[tail.len() - 1])
        {
            text.truncate(start);
        }
    }
    let lower = text.trim_end().to_owned();
    for marker in [" season", "season", " part", " s"] {
        if let Some(at) = lower.rfind(marker) {
            let tail = &lower[at + marker.len()..].trim();
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                return lower[..at].trim_end().to_owned();
            }
        }
    }
    if let Some((head, tail)) = lower
        .rsplit_once(' ')
        .filter(|(_, tail)| ROMAN.contains(tail))
    {
        let _ = tail;
        return head.to_owned();
    }
    lower
        .trim_end_matches(|c: char| c.is_ascii_digit() || "一二三四五六七八九十".contains(c))
        .trim_end()
        .to_owned()
}

/// Queries to try in order: as typed, normalized, without sequel markers, then a shorter prefix.
pub fn relaxed(query: &str) -> Vec<String> {
    let typed = query.trim().to_owned();
    let normalized = normalize(&typed);
    let base = without_sequel(&normalized);
    let chars: Vec<char> = base.chars().collect();
    let prefix = if base.contains(' ') {
        base.split(' ').next().unwrap_or("").to_owned()
    } else if chars.len() >= 4 && !base.is_ascii() {
        // Only CJK titles shorten by characters; a single English word stays whole.
        chars[..chars.len().div_ceil(2).max(2)].iter().collect()
    } else {
        String::new()
    };
    let mut out: Vec<String> = Vec::new();
    for (index, candidate) in [typed, normalized, base, prefix].into_iter().enumerate() {
        let short =
            candidate.chars().count() < 2 && !candidate.chars().all(|c| c.is_ascii_alphanumeric());
        if !candidate.is_empty() && (index == 0 || !short) && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// Characters of `query` found in `title`, in order (longest common subsequence length).
fn common(query: &[char], title: &[char]) -> usize {
    let mut row = vec![0usize; title.len() + 1];
    for &q in query {
        let mut previous = 0;
        for (j, &t) in title.iter().enumerate() {
            let saved = row[j + 1];
            row[j + 1] = if q == t {
                previous + 1
            } else {
                row[j + 1].max(row[j])
            };
            previous = saved;
        }
    }
    row[title.len()]
}

/// Lower is closer: exact, then prefix, then containment, then shared characters.
pub fn score(query: &str, title: &str) -> f64 {
    let (q, t) = (normalize(query), normalize(title));
    if q.is_empty() {
        return 4.0;
    }
    if t == q {
        return 0.0;
    }
    if t.starts_with(&q) {
        return 1.0 + (t.chars().count() - q.chars().count()) as f64 / 1000.0;
    }
    if t.contains(&q) {
        return 2.0;
    }
    let (qc, tc): (Vec<char>, Vec<char>) = (q.chars().collect(), t.chars().collect());
    3.0 + 1.0 - common(&qc, &tc) as f64 / qc.len() as f64
}

/// Whether a search result title names the queried title at all. Some MacCMS
/// sites answer `wd` with full-text relevance (descriptions, cast), so an
/// unrelated title can come back first; matching keeps only containment.
pub fn related(query: &str, title: &str) -> bool {
    let (q, t) = (normalize(query), normalize(title));
    !q.is_empty() && !t.is_empty() && (t.contains(&q) || t.chars().count() >= 2 && q.contains(&t))
}

/// Stable sort of search cards by closeness of their titles to the typed query.
pub fn rank(query: &str, items: &mut [Value]) {
    items.sort_by(|a, b| {
        let title = |v: &Value| v["title"].as_str().unwrap_or("").to_owned();
        score(query, &title(a)).total_cmp(&score(query, &title(b)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_relax_and_rank() {
        assert_eq!(
            normalize("  复仇者联盟　４：终局之战 "),
            "复仇者联盟4终局之战"
        );
        assert_eq!(normalize("Iron-Man  3"), "iron man 3");
        assert_eq!(
            relaxed("复仇者联盟 4"),
            vec!["复仇者联盟 4", "复仇者联盟4", "复仇者联盟", "复仇者"]
        );
        assert_eq!(
            relaxed("庆余年 第二季"),
            vec!["庆余年 第二季", "庆余年第二季", "庆余年"]
        );
        assert_eq!(
            relaxed("Breaking Bad Season 2"),
            vec![
                "Breaking Bad Season 2",
                "breaking bad season 2",
                "breaking bad",
                "breaking"
            ]
        );
        assert_eq!(
            relaxed("Rocky III"),
            vec!["Rocky III", "rocky iii", "rocky"]
        );
        assert_eq!(relaxed("流浪地球2"), vec!["流浪地球2", "流浪地球", "流浪"]);
        assert!(relaxed("  ").is_empty());
        assert_eq!(relaxed("龙"), vec!["龙"]);
        let mut items = vec![
            json!({"title": "地球脉动"}),
            json!({"title": "流浪地球2"}),
            json!({"title": "流浪地球"}),
            json!({"title": "我的流浪地球日记"}),
        ];
        rank("流浪地球", &mut items);
        let order: Vec<_> = items.iter().map(|i| i["title"].as_str().unwrap()).collect();
        assert_eq!(
            order,
            vec!["流浪地球", "流浪地球2", "我的流浪地球日记", "地球脉动"]
        );
    }

    #[test]
    fn related_titles_only() {
        // Full-text results seen from a real source for "The Odyssey" / "奥德赛".
        for title in [
            "荷马史诗：神话背后第一季",
            "西游降魔篇",
            "海上医疗队",
            "2001太空漫游",
        ] {
            assert!(
                !related("The Odyssey", title) && !related("奥德赛", title),
                "{title}"
            );
        }
        for title in ["奥德赛", "奥德赛2026", "尤利西斯：黑暗的奥德赛"] {
            assert!(related("奥德赛", title), "{title}");
        }
        assert!(related("The Odyssey", "the odyssey"));
        assert!(related("奥德赛 2026", "奥德赛"));
        assert!(!related("奥德赛", "") && !related("", "奥德赛") && !related("奥德赛", "赛"));
    }
}
