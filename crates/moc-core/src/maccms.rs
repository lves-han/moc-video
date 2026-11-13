#[derive(Debug, PartialEq, Eq)]
pub struct Episode<'a> {
    pub label: &'a str,
    pub url: &'a str,
    pub raw: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Group<'a> {
    pub name: &'a str,
    pub episodes: Vec<Episode<'a>>,
}

pub fn playback<'a>(names: &'a str, urls: &'a str) -> Result<Vec<Group<'a>>, &'static str> {
    if names.is_empty() && urls.is_empty() {
        return Ok(Vec::new());
    }
    let names: Vec<_> = names.split("$$$").collect();
    let groups: Vec<_> = urls.split("$$$").collect();
    if names.len() != groups.len()
        || names.iter().any(|s| s.is_empty())
        || groups.iter().any(|s| s.is_empty())
    {
        return Err("MacCMS playback groups are ambiguous");
    }
    names
        .into_iter()
        .zip(groups)
        .map(|(name, group)| {
            let episodes = group
                .split('#')
                .map(|entry| {
                    if entry.is_empty() {
                        return Err("MacCMS playback contains a missing episode");
                    }
                    let (label, url) =
                        if entry.starts_with("http://") || entry.starts_with("https://") {
                            ("", entry)
                        } else {
                            entry.split_once('$').unwrap_or(("", entry))
                        };
                    if url.is_empty() {
                        return Err("MacCMS episode resource is missing");
                    }
                    Ok(Episode {
                        label,
                        url,
                        raw: entry,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Group { name, episodes })
        })
        .collect()
}
