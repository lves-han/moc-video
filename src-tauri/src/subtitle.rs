//! Bounded UTF-8 subtitles. ASS is reduced to timed plain text: typography,
//! positioning, animation and vector drawings are deliberately omitted.
use crate::contracts::ApiError;

pub const LIMIT: usize = 8 * 1024 * 1024;

fn invalid(message: &str) -> ApiError {
    ApiError::new("UPSTREAM_FAILED", message)
        .with_details(serde_json::json!({"reason":"invalid_subtitle"}))
}

fn timestamp(value: &str, separator: char, fraction: usize, short: bool) -> Result<u64, ApiError> {
    let (clock, decimal) = value
        .split_once(separator)
        .ok_or_else(|| invalid("Subtitle timestamp lacks a fraction"))?;
    let parts: Vec<_> = clock.split(':').collect();
    if !(parts.len() == 3 || short && parts.len() == 2)
        || decimal.len() != fraction
        || !decimal.bytes().all(|b| b.is_ascii_digit())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        || parts[parts.len() - 2..].iter().any(|p| p.len() != 2)
    {
        return Err(invalid("Invalid subtitle timestamp"));
    }
    let numbers: Vec<u64> = parts
        .iter()
        .map(|p| {
            p.parse()
                .map_err(|_| invalid("Subtitle timestamp overflow"))
        })
        .collect::<Result<_, _>>()?;
    let minutes = numbers[numbers.len() - 2];
    let seconds = numbers[numbers.len() - 1];
    if minutes >= 60 || seconds >= 60 {
        return Err(invalid("Subtitle minute or second out of range"));
    }
    let hours = if numbers.len() == 3 { numbers[0] } else { 0 };
    hours
        .checked_mul(3_600_000)
        .and_then(|n| n.checked_add(minutes * 60_000 + seconds * 1000))
        .and_then(|n| {
            n.checked_add(decimal.parse::<u64>().ok()? * if fraction == 2 { 10 } else { 1 })
        })
        .ok_or_else(|| invalid("Subtitle timestamp overflow"))
}

fn vtt_time(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// Remove only recognized SRT styling. Literal comparisons and unknown angle
// brackets survive as text; a broad <.*> replacement would destroy dialogue.
fn srt_text(text: &str) -> String {
    let mut plain = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with('<')
            && let Some(end) = rest[1..].find(['<', '>']).map(|n| n + 1)
            && rest.as_bytes()[end] == b'>'
        {
            let tag = rest[1..end]
                .trim()
                .trim_start_matches('/')
                .to_ascii_lowercase();
            if matches!(tag.as_str(), "b" | "i" | "u" | "s" | "font") || tag.starts_with("font ") {
                rest = &rest[end + 1..];
                continue;
            }
        }
        if rest.starts_with('&')
            && let Some(end) = rest.as_bytes().iter().take(13).position(|b| *b == b';')
        {
            let entity = &rest[1..end];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|n| u32::from_str_radix(n, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|n| n.parse().ok()))
                    .and_then(char::from_u32)
                    .filter(|c| !c.is_control()),
            };
            if let Some(c) = decoded {
                plain.push(c);
                rest = &rest[end + 1..];
                continue;
            }
        }
        let c = rest.chars().next().expect("Nonempty text");
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    escape(&plain)
}

fn cue(output: &mut String, start: u64, end: u64, text: &str) -> Result<(), ApiError> {
    if end <= start {
        return Err(invalid("Subtitle cue end must follow start"));
    }
    if !text.is_empty() {
        output.push_str(&format!(
            "{} --> {}\n{}\n\n",
            vtt_time(start),
            vtt_time(end),
            text
        ));
    }
    if output.len() > LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Converted subtitle exceeds 8 MiB",
        ));
    }
    Ok(())
}

fn timed(text: &str, vtt: bool) -> Result<String, ApiError> {
    let mut output = String::from("WEBVTT\n\n");
    let mut count = 0;
    let mut previous = 0;
    let mut srt_cues = Vec::new();
    let body = if vtt {
        let (header, body) = text
            .split_once("\n\n")
            .ok_or_else(|| invalid("WebVTT header requires a blank line"))?;
        if header.contains("-->") {
            return Err(invalid("Invalid WebVTT header"));
        }
        output = format!("{header}\n\n");
        body
    } else {
        text
    };
    for block in body.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let block = block.trim_matches('\n');
        let lines: Vec<_> = block.split('\n').collect();
        if vtt
            && (lines[0] == "NOTE"
                || lines[0].starts_with("NOTE ")
                || lines[0].starts_with("NOTE\t")
                || matches!(lines[0], "STYLE" | "REGION"))
        {
            output.push_str(block);
            output.push_str("\n\n");
            continue;
        }
        let timing = usize::from(!lines[0].contains("-->"));
        if timing > 0 && !vtt && !lines[0].bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("Invalid subtitle cue identifier"));
        }
        let line = lines
            .get(timing)
            .ok_or_else(|| invalid("Subtitle cue lacks timing"))?;
        let (a, b) = line
            .split_once("-->")
            .ok_or_else(|| invalid("Subtitle cue lacks timing arrow"))?;
        let fields: Vec<_> = b.split_whitespace().collect();
        if fields.is_empty() || !vtt && fields.len() != 1 {
            return Err(invalid("Invalid subtitle cue timing"));
        }
        let start = timestamp(a.trim(), if vtt { '.' } else { ',' }, 3, vtt)?;
        let end = timestamp(fields[0], if vtt { '.' } else { ',' }, 3, vtt)?;
        if vtt && start < previous {
            return Err(invalid("WebVTT cue start times must be ordered"));
        }
        previous = start;
        if lines.len() <= timing + 1 {
            return Err(invalid("Subtitle cue lacks text"));
        }
        let payload = lines[timing + 1..].join("\n");
        if vtt {
            if end <= start || payload.contains("-->") {
                return Err(invalid("Invalid WebVTT cue"));
            }
            // Preserve cue IDs, settings, markup and timestamp tags verbatim.
            output.push_str(block.trim_end_matches('\n'));
            output.push_str("\n\n");
        } else {
            if end <= start {
                return Err(invalid("Subtitle cue end must follow start"));
            }
            srt_cues.push((start, end, srt_text(&payload)));
        }
        count += 1;
    }
    if count == 0 && !vtt {
        return Err(invalid("Subtitle contains no cues"));
    }
    // SRT numbering does not establish chronology. WebVTT requires cues in
    // start-time order; sorting preserves overlaps and all original timestamps.
    srt_cues.sort_by_key(|c| c.0);
    for (start, end, text) in srt_cues {
        cue(&mut output, start, end, &text)?;
    }
    Ok(output)
}

fn ass_text(text: &str, wrap: u8) -> Result<String, ApiError> {
    let mut plain = String::new();
    let mut rest = text;
    let mut drawing = false;
    let mut wrap = wrap;
    while !rest.is_empty() {
        if rest.starts_with('{')
            && let Some(end) = rest[1..].find(['{', '}']).map(|n| n + 1)
            && rest.as_bytes()[end] == b'}'
        {
            let block = &rest[1..end];
            if block.contains('\\') {
                for tag in block.split('\\').skip(1) {
                    if let Some(n) = tag
                        .strip_prefix('p')
                        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                    {
                        drawing = n
                            .parse::<u32>()
                            .map_err(|_| invalid("ASS drawing mode overflow"))?
                            > 0;
                    }
                    if let Some(n) = tag
                        .strip_prefix('q')
                        .filter(|n| matches!(*n, "0" | "1" | "2" | "3"))
                    {
                        wrap = n.parse().expect("ASS wrap mode");
                    }
                    if tag.starts_with('r') {
                        drawing = false;
                    }
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        let c = rest.chars().next().expect("Nonempty ASS text");
        rest = &rest[c.len_utf8()..];
        if c == '\\'
            && let Some(next) = rest.chars().next()
        {
            let mapped = match next {
                'N' => Some('\n'),
                'n' => Some(if wrap == 2 { '\n' } else { ' ' }),
                'h' => Some('\u{a0}'),
                '{' | '}' | '\\' => Some(next),
                _ => None,
            };
            if let Some(mapped) = mapped {
                if !drawing {
                    plain.push(mapped);
                }
                rest = &rest[next.len_utf8()..];
                continue;
            }
        }
        if !drawing {
            plain.push(c);
        }
    }
    Ok(escape(&plain))
}

fn ass(text: &str) -> Result<String, ApiError> {
    let mut section = String::new();
    let mut fields: Vec<String> = Vec::new();
    let mut wrap = 0;
    let mut cues = Vec::new();
    for line in text
        .lines()
        .map(str::trim_start)
        .filter(|l| !l.trim().is_empty() && !l.starts_with(';'))
    {
        if line.starts_with('[') && line.trim_end().ends_with(']') {
            section = line.trim_end().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        if section == "[script info]" && key == "wrapstyle" {
            wrap = value
                .trim()
                .parse()
                .map_err(|_| invalid("Invalid ASS WrapStyle"))?;
            if wrap > 3 {
                return Err(invalid("Invalid ASS WrapStyle"));
            }
        }
        if section != "[events]" {
            continue;
        }
        if key == "format" {
            fields = value
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .collect();
            if fields.last().map(String::as_str) != Some("text")
                || fields.len() < 3
                || fields.iter().filter(|f| *f == "start").count() != 1
                || fields.iter().filter(|f| *f == "end").count() != 1
                || fields.iter().filter(|f| *f == "text").count() != 1
            {
                return Err(invalid(
                    "ASS Events Format requires unique Start/End and final Text",
                ));
            }
        } else if key == "dialogue" {
            if fields.is_empty() {
                return Err(invalid("ASS Dialogue requires Events Format"));
            }
            let values: Vec<_> = value.trim_start().splitn(fields.len(), ',').collect();
            if values.len() != fields.len() {
                return Err(invalid("ASS Dialogue field count differs from Format"));
            }
            let start = timestamp(
                values[fields
                    .iter()
                    .position(|f| f == "start")
                    .expect("Start field")]
                .trim(),
                '.',
                2,
                false,
            )?;
            let end = timestamp(
                values[fields.iter().position(|f| f == "end").expect("End field")].trim(),
                '.',
                2,
                false,
            )?;
            if end <= start {
                return Err(invalid("ASS cue end must follow start"));
            }
            cues.push((start, end, ass_text(values[values.len() - 1], wrap)?));
        }
    }
    if cues.is_empty() {
        return Err(invalid("ASS contains no Dialogue events"));
    }
    cues.sort_by_key(|c| c.0);
    let mut output = String::from("WEBVTT\n\n");
    for (start, end, text) in cues {
        cue(&mut output, start, end, &text)?;
    }
    Ok(output)
}

pub fn convert(bytes: &[u8], format: &str) -> Result<Vec<u8>, ApiError> {
    if bytes.len() > LIMIT {
        return Err(ApiError::new("RESOURCE_LIMIT", "Subtitle exceeds 8 MiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("Subtitle must be UTF-8"))?
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if text.contains('\0') {
        return Err(invalid("Subtitle contains NUL"));
    }
    let detected = if text.starts_with("WEBVTT\n")
        || text.starts_with("WEBVTT ")
        || text.starts_with("WEBVTT\t")
    {
        "vtt"
    } else if text
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case("[events]"))
    {
        "ass"
    } else {
        "srt"
    };
    let format = if format == "unknown" {
        detected
    } else {
        format
    };
    let output = match format {
        "vtt" if detected == "vtt" => timed(&text, true)?,
        "srt" => timed(&text, false)?,
        "ass" => ass(&text)?,
        _ => return Err(invalid("Unsupported or mismatched subtitle format")),
    };
    if output.len() > LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Converted subtitle exceeds 8 MiB",
        ));
    }
    Ok(output.into_bytes())
}
