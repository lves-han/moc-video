//! Lossless URI spans: tags, attribute ordering, IVs and timeline data remain
//! byte-for-byte intact. Only parsed URI values are replaced by registered IDs.
use crate::contracts::ApiError;
use std::ops::Range;

pub(super) const LIMIT: usize = 8 * 1024 * 1024;

pub(super) struct Reference {
    pub span: Range<usize>,
    pub playlist: bool,
}

fn invalid() -> ApiError {
    ApiError::new("UPSTREAM_FAILED", "Invalid HLS manifest syntax")
}

/// Attribute values are scanned with quote awareness; commas inside quoted
/// strings and strings containing the text URI= never become delimiters.
fn attributes(text: &str) -> Result<Vec<(&str, Range<usize>)>, ApiError> {
    let bytes = text.as_bytes();
    let mut found: Vec<(&str, Range<usize>)> = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let start = pos;
        while pos < bytes.len() && bytes[pos] != b'=' && bytes[pos] != b',' {
            pos += 1;
        }
        if pos == bytes.len() || bytes[pos] != b'=' {
            return Err(invalid());
        }
        let name = &text[start..pos];
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            || found.iter().any(|(old, _)| *old == name)
        {
            return Err(invalid());
        }
        pos += 1;
        let quoted = bytes.get(pos) == Some(&b'"');
        if quoted {
            pos += 1;
        }
        let value = pos;
        while pos < bytes.len() && bytes[pos] != if quoted { b'"' } else { b',' } {
            pos += 1;
        }
        let end = pos;
        if quoted {
            if bytes.get(pos) != Some(&b'"') {
                return Err(invalid());
            }
            pos += 1;
        }
        if pos < bytes.len() && bytes[pos] != b',' {
            return Err(invalid());
        }
        if (name == "URI" || name.ends_with("-URI")) && (!quoted || value == end) {
            return Err(invalid());
        }
        found.push((name, value..end));
        if pos < bytes.len() {
            pos += 1;
            if pos == bytes.len() {
                return Err(invalid());
            }
        }
    }
    Ok(found)
}

pub(super) fn references(text: &str) -> Result<Vec<Reference>, ApiError> {
    if text.len() > LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "HLS manifest exceeds 8 MiB",
        ));
    }
    if text.lines().next() != Some("#EXTM3U")
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
    {
        return Err(invalid());
    }
    let mut refs = Vec::new();
    let mut offset = 0;
    let mut variant = false;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\r', '\n']);
        if !line.is_empty() && !line.starts_with('#') {
            if line.trim() != line || line.contains("{$") {
                return Err(invalid());
            }
            refs.push(Reference {
                span: offset..offset + line.len(),
                playlist: variant,
            });
            variant = false;
        } else if let Some((tag, values)) = line.split_once(':') {
            if tag == "#EXT-X-STREAM-INF" {
                variant = true;
            }
            // Numeric/time tags and EXTINF have no attribute list. Unknown
            // extension lists are parsed too, including SERVER-URI extensions.
            let attribute_list = values.split_once('=').is_some_and(|(name, _)| {
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            });
            if tag.starts_with("#EXT-X-")
                && (attribute_list
                    || matches!(
                        tag,
                        "#EXT-X-KEY"
                            | "#EXT-X-SESSION-KEY"
                            | "#EXT-X-MEDIA"
                            | "#EXT-X-MAP"
                            | "#EXT-X-STREAM-INF"
                            | "#EXT-X-I-FRAME-STREAM-INF"
                            | "#EXT-X-PART"
                            | "#EXT-X-PRELOAD-HINT"
                            | "#EXT-X-RENDITION-REPORT"
                    ))
            {
                let attrs = attributes(values)?;
                if matches!(tag, "#EXT-X-KEY" | "#EXT-X-SESSION-KEY") {
                    let value = |name| {
                        attrs
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, r)| &values[r.clone()])
                    };
                    if !matches!(value("METHOD"), Some("NONE" | "AES-128"))
                        || value("KEYFORMAT").is_some_and(|v| v != "identity")
                    {
                        let drm_type = match value("KEYFORMAT") {
                            Some("com.apple.streamingkeydelivery") => "fairplay",
                            Some("urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed") => "widevine",
                            Some(
                                "com.microsoft.playready"
                                | "urn:uuid:9a04f079-9840-4286-ab92-e65be0885f95",
                            ) => "playready",
                            Some("identity") | None => "unsupported_encryption",
                            _ => "unknown",
                        };
                        return Err(ApiError::new(
                            "MEDIA_UNSUPPORTED",
                            "HLS DRM or encryption method is unsupported",
                        )
                        .with_details(
                            serde_json::json!({"media_type":"hls", "drm_type":drm_type}),
                        ));
                    }
                    if value("METHOD") == Some("AES-128") && value("URI").is_none() {
                        return Err(invalid());
                    }
                }
                let start = offset + tag.len() + 1;
                for (name, span) in attrs {
                    if name == "URI" || name.ends_with("-URI") {
                        if values[span.clone()].contains("{$") {
                            return Err(invalid());
                        }
                        refs.push(Reference {
                            span: start + span.start..start + span.end,
                            playlist: matches!(
                                tag,
                                "#EXT-X-MEDIA"
                                    | "#EXT-X-I-FRAME-STREAM-INF"
                                    | "#EXT-X-RENDITION-REPORT"
                            ),
                        });
                    }
                }
            }
        }
        offset += raw.len();
    }
    if variant {
        return Err(invalid());
    }
    Ok(refs)
}
