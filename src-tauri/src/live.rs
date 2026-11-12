//! Live channel lists: IPTV M3U/M3U8 playlists and "name,url" TXT lists.
//! The format is detected from the content, so one setting accepts either.
use crate::contracts::ApiError;
use serde_json::{Value, json};

pub const LIST_LIMIT: usize = 8 * 1024 * 1024;
// Large public lists (for example iptv-org's full index) exceed 11 000 channels.
const CHANNEL_LIMIT: usize = 50_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub name: String,
    pub group: String,
    pub logo: Option<String>,
    pub url: String,
}

impl Channel {
    /// Only http(s) reaches the media gateway; multicast/RTMP addresses stay listed but unplayable.
    pub fn supported(&self) -> bool {
        self.url.starts_with("http://") || self.url.starts_with("https://")
    }
    pub fn view(&self, index: usize) -> Value {
        json!({"index": index, "name": self.name, "group": self.group, "logo": self.logo,
            "url": self.url, "supported": self.supported()})
    }
}

fn unrecognized() -> ApiError {
    ApiError::new(
        "UPSTREAM_FAILED",
        "Live list is neither an M3U playlist nor a name,url text list",
    )
}

fn clip(value: &str, limit: usize) -> String {
    value.trim().chars().take(limit).collect()
}

/// Returns the detected format ("m3u" or "txt") and the channels in list order.
pub fn parse(body: &str) -> Result<(&'static str, Vec<Channel>), ApiError> {
    let body = body.trim_start_matches('\u{feff}');
    let channels = if body.trim_start().starts_with("#EXTM3U") || body.contains("#EXTINF") {
        ("m3u", parse_m3u(body))
    } else {
        ("txt", parse_txt(body))
    };
    if channels.1.is_empty() {
        return Err(unrecognized());
    }
    if channels.1.len() > CHANNEL_LIMIT {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Live list exceeds 50000 channels",
        ));
    }
    Ok(channels)
}

fn attribute(line: &str, name: &str) -> Option<String> {
    let start = line.find(&format!("{name}=\""))? + name.len() + 2;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_owned()).filter(|v| !v.trim().is_empty())
}

fn parse_m3u(body: &str) -> Vec<Channel> {
    let mut channels = Vec::new();
    let mut pending: Option<(String, String, Option<String>)> = None;
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            // The display name follows the last comma outside quoted attributes.
            let mut quoted = false;
            let mut split = None;
            for (index, c) in info.char_indices() {
                match c {
                    '"' => quoted = !quoted,
                    ',' if !quoted => split = Some(index),
                    _ => {}
                }
            }
            let name = split.map(|i| &info[i + 1..]).unwrap_or("");
            let group = attribute(info, "group-title").unwrap_or_default();
            let fallback = attribute(info, "tvg-name").unwrap_or_default();
            let name = if name.trim().is_empty() { fallback } else { name.to_owned() };
            pending = Some((name, group, attribute(info, "tvg-logo")));
        } else if let Some(group) = line.strip_prefix("#EXTGRP:") {
            if let Some(entry) = &mut pending
                && entry.1.is_empty() {
                    entry.1 = group.trim().to_owned();
                }
        } else if !line.starts_with('#') {
            let (name, group, logo) = pending.take().unwrap_or_default();
            let name = if name.trim().is_empty() { line.to_owned() } else { name };
            channels.push(Channel {
                name: clip(&name, 256),
                group: clip(&group, 256),
                logo: logo.map(|l| clip(&l, 2048)),
                url: clip(line, 4096),
            });
        }
    }
    channels
}

fn parse_txt(body: &str) -> Vec<Channel> {
    let mut channels = Vec::new();
    let mut group = String::new();
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with("//")) {
        let Some((name, rest)) = line.split_once(',') else {
            continue;
        };
        let rest = rest.trim();
        if rest == "#genre#" {
            group = name.trim().to_owned();
            continue;
        }
        // Lists often join alternative addresses with '#'; the first one is the channel.
        let url = rest.split('#').next().unwrap_or("").trim();
        if name.trim().is_empty() || !url.contains("://") {
            continue;
        }
        channels.push(Channel {
            name: clip(name, 256),
            group: clip(&group, 256),
            logo: None,
            url: clip(url, 4096),
        });
    }
    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m3u_groups_logos_names_and_unsupported_protocols() {
        let body = "\u{feff}#EXTM3U x-tvg-url=\"https://epg.test/e.xml\"\n\
            #EXTINF:-1 tvg-id=\"1\" tvg-logo=\"https://logo.test/1.png\" group-title=\"央视, 频道\",CCTV-1 综合\n\
            https://live.test/cctv1.m3u8\n\
            #EXTINF:-1 tvg-name=\"Backup\" group-title=\"卫视\",\n\
            #EXTVLCOPT:http-user-agent=x\n\
            http://live.test/b.m3u8\n\
            #EXTINF:-1,Multicast\n#EXTGRP:组播\n\
            rtp://239.3.1.1:8000\n";
        let (format, channels) = parse(body).unwrap();
        assert_eq!(format, "m3u");
        assert_eq!(channels.len(), 3);
        assert_eq!(channels[0], Channel { name: "CCTV-1 综合".into(), group: "央视, 频道".into(),
            logo: Some("https://logo.test/1.png".into()), url: "https://live.test/cctv1.m3u8".into() });
        assert_eq!((channels[1].name.as_str(), channels[1].group.as_str()), ("Backup", "卫视"));
        assert_eq!(channels[2].group, "组播");
        assert!(channels[0].supported() && channels[1].supported() && !channels[2].supported());
        assert_eq!(channels[2].view(2)["supported"], false);
    }

    #[test]
    fn txt_genre_groups_alternatives_and_skipped_lines() {
        let body = "央视频道,#genre#\nCCTV-1,http://a.test/1.m3u8#http://b.test/1.m3u8\nCCTV-2,https://a.test/2.m3u8\n\
            // comment\nbroken line\n,http://a.test/x\n卫视频道,#genre#\n湖南卫视,rtmp://a.test/hn\n";
        let (format, channels) = parse(body).unwrap();
        assert_eq!(format, "txt");
        assert_eq!(channels.iter().map(|c| (c.group.as_str(), c.name.as_str())).collect::<Vec<_>>(),
            vec![("央视频道", "CCTV-1"), ("央视频道", "CCTV-2"), ("卫视频道", "湖南卫视")]);
        assert_eq!(channels[0].url, "http://a.test/1.m3u8");
        assert!(!channels[2].supported());
    }

    #[test]
    fn unrecognized_and_oversized_lists_fail_explicitly() {
        for body in ["", "<html><body>Not found</body></html>", "just,text\nno urls here"] {
            assert_eq!(parse(body).unwrap_err().code, "UPSTREAM_FAILED", "{body}");
        }
        let many: String = (0..50_001).map(|i| format!("c{i},http://a.test/{i}\n")).collect();
        assert_eq!(parse(&many).unwrap_err().code, "RESOURCE_LIMIT");
    }
}
