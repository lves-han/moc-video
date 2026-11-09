//! Cross-source confirmation. Native evidence never comes from invoke input.
use crate::{
    contracts::{ApiError, Contracts},
    requests::Job,
    source_dispatch::SourceSnapshot,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

pub const LEASE_MS: u64 = 300_000;
const MAX_RECORDS: usize = 256;

#[derive(Clone)]
pub struct Candidate {
    pub details: Value,
    /// Set only by an adapter which has verified both ID namespace and type.
    pub external_identity: Option<(String, String, String)>,
    /// Verified metadata video ID by native episode ID, when a provider exposes
    /// an explicit external video identity instead of season/episode numbers.
    pub external_videos: HashMap<String, String>,
}

pub fn candidate(details: &Value) -> Value {
    json!({"content_ref":details["ref"], "title":details["title"], "year":details["year"]})
}

pub fn video(metadata: &Value, id: &str) -> Result<Value, ApiError> {
    let mut found = Vec::new();
    let maccms_id = if metadata["metadata_ref"]["identity"]["namespace"] == "maccms" {
        Some(
            serde_json::from_str::<[String; 4]>(id)
                .map_err(|_| ApiError::bad("Invalid MacCMS video identity"))?,
        )
    } else {
        None
    };
    for group in metadata["details"]["groups"]
        .as_array()
        .ok_or_else(|| ApiError::bad("Metadata groups are missing"))?
    {
        for episode in group["episodes"]
            .as_array()
            .ok_or_else(|| ApiError::bad("Metadata episodes are missing"))?
        {
            let fields: Vec<String> =
                serde_json::from_str(episode["ref"]["data"].as_str().unwrap_or(""))
                    .map_err(|_| ApiError::bad("Metadata video reference is invalid"))?;
            let same_maccms = fields.len() == 5
                && fields[0] == "maccms_episode"
                && maccms_id
                    .as_ref()
                    .is_some_and(|video| video[0] == "maccms_video" && video[1..] == fields[1..4]);
            if same_maccms || (fields.len() == 5 && fields[0] == "stremio_video" && fields[4] == id)
            {
                found.push(episode.clone());
            }
        }
    }
    if found.len() != 1 {
        return Err(ApiError::bad("Requested video is missing or ambiguous"));
    }
    Ok(found.remove(0))
}

pub fn selection(details: &Value, selection: &Value) -> Result<Value, ApiError> {
    if details["ref"] != selection["content_ref"] {
        return Err(ApiError::bad("Selection content is not registered"));
    }
    for group in details["groups"]
        .as_array()
        .ok_or_else(|| ApiError::bad("Selection groups are missing"))?
    {
        if group["id"] != selection["group_id"] {
            continue;
        }
        for episode in group["episodes"].as_array().expect("Validated details") {
            if episode["id"] == selection["episode_id"] {
                return Ok(episode.clone());
            }
        }
    }
    Err(ApiError::bad(
        "Selection episode does not belong to this content and group",
    ))
}

pub struct Proposal {
    pub candidates: Vec<Value>,
    pub selected: Option<(Value, Value)>,
}

/// Two queries, ten results each, stable deduplication. A failed query retains
/// its error even if another query returned candidates or an empty page.
pub fn search(
    metadata: &Value,
    requested_video: &Value,
    aliases: &[String],
    job: &Job,
    mut query: impl FnMut(&str, &Job) -> Result<Vec<Candidate>, ApiError>,
) -> Result<Proposal, ApiError> {
    let mut queries = Vec::new();
    for title in std::iter::once(
        metadata["details"]["title"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
    )
    .chain(aliases.iter().cloned())
    {
        let key = title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if !key.is_empty() && !queries.iter().any(|(old, _)| old == &key) {
            queries.push((key, title));
        }
        if queries.len() == 2 {
            break;
        }
    }
    let mut found: Vec<Candidate> = Vec::new();
    for (_, title) in queries {
        budget(job)?;
        let child = job.aggregate_child(&Uuid::new_v4().to_string(), "default_metadata")?;
        for item in query(&title, &child)?.into_iter().take(10) {
            if !found
                .iter()
                .any(|old| old.details["ref"] == item.details["ref"])
            {
                found.push(item);
            }
        }
        budget(job)?;
    }
    let identity = &metadata["metadata_ref"]["identity"];
    let video_fields: Vec<String> =
        serde_json::from_str(requested_video["ref"]["data"].as_str().unwrap_or(""))
            .unwrap_or_default();
    let video_id = (video_fields.len() == 5 && video_fields[0] == "stremio_video")
        .then(|| video_fields[4].as_str());
    let mut exact = Vec::new();
    let mut verified_contents = 0;
    for item in &found {
        let verified = item
            .external_identity
            .as_ref()
            .is_some_and(|(namespace, id, kind)| {
                matches!(namespace.as_str(), "imdb" | "tmdb")
                    && identity["namespace"] == *namespace
                    && identity["canonical_id"] == *id
                    && identity["type"] == *kind
                    && item.details["kind"] == *kind
            });
        if !verified {
            continue;
        }
        verified_contents += 1;
        let movie = identity["type"] == "movie";
        for group in item.details["groups"]
            .as_array()
            .ok_or_else(|| ApiError::bad("Provider groups are missing"))?
        {
            for episode in group["episodes"]
                .as_array()
                .ok_or_else(|| ApiError::bad("Provider episodes are missing"))?
            {
                let external_video = episode["id"]
                    .as_str()
                    .and_then(|id| item.external_videos.get(id))
                    .is_some_and(|id| Some(id.as_str()) == video_id);
                if movie
                    || external_video
                    || (!requested_video["season"].is_null()
                        && !requested_video["number"].is_null()
                        && episode["season"] == requested_video["season"]
                        && episode["number"].as_f64() == requested_video["number"].as_f64())
                {
                    exact.push((item.details.clone(), json!({"content_ref":item.details["ref"], "group_id":group["id"], "episode_id":episode["id"]})));
                }
            }
        }
    }
    let selected = if verified_contents == 1 && exact.len() == 1 {
        exact.pop()
    } else {
        None
    };
    Ok(Proposal {
        candidates: found.iter().map(|item| candidate(&item.details)).collect(),
        selected,
    })
}

pub fn budget(job: &Job) -> Result<(), ApiError> {
    if job.aggregate_remaining_ms()? == 0 {
        return Err(ApiError::new(
            "UPSTREAM_TIMEOUT",
            "Matching finalization deadline exhausted",
        ));
    }
    Ok(())
}

#[derive(Clone)]
pub struct MatchRecord {
    pub context: Value,
    pub metadata: SourceSnapshot,
    pub provider: SourceSnapshot,
    pub confirmation: &'static str,
    pub label: String,
    touched: u64,
}

#[derive(Default)]
pub struct MatchStore {
    records: HashMap<String, MatchRecord>,
    details: HashMap<String, (Value, u64)>,
    sequence: u64,
    scope: Value,
}

impl MatchStore {
    pub fn begin(&mut self, input: &Value) -> u64 {
        let scope = json!([
            input["metadata_ref"],
            input["video_id"],
            input["playback_source"]
        ]);
        if self.scope != scope {
            self.records.clear();
            self.scope = scope;
        }
        self.sequence += 1;
        self.sequence
    }
    pub fn check(&self, sequence: u64) -> Result<(), ApiError> {
        if self.sequence != sequence {
            return Err(ApiError::cancelled());
        }
        Ok(())
    }
    pub fn revoke(&mut self, sources: &[String]) {
        self.records.retain(|_, r| {
            !sources.contains(&r.metadata.source_id) && !sources.contains(&r.provider.source_id)
        });
        self.details
            .retain(|_, (d, _)| !sources.iter().any(|id| d["ref"]["source_id"] == *id));
    }
    pub fn release(&mut self, id: &str) {
        self.records.remove(id);
    }
    pub fn invalidate(&mut self) {
        self.records.clear();
        self.sequence += 1;
        self.scope = Value::Null;
    }
    pub fn register_details(&mut self, details: Value, now: u64) {
        self.expire(now);
        if self.details.len() >= MAX_RECORDS {
            let oldest = self
                .details
                .iter()
                .min_by_key(|(_, (_, time))| *time)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                self.details.remove(&key);
            }
        }
        self.details
            .insert(details["ref"].to_string(), (details, now));
    }
    pub fn registered(&mut self, selection: &Value, now: u64) -> Result<Value, ApiError> {
        self.expire(now);
        let (details, touched) = self
            .details
            .get_mut(&selection["content_ref"].to_string())
            .ok_or_else(|| ApiError::bad("Inspect current provider details before confirming"))?;
        super::matching::selection(details, selection)?;
        *touched = now;
        Ok(details.clone())
    }
    fn expire(&mut self, now: u64) {
        self.records
            .retain(|_, r| now.saturating_sub(r.touched) < LEASE_MS);
        self.details
            .retain(|_, (_, time)| now.saturating_sub(*time) < LEASE_MS);
    }
    pub fn issue(
        &mut self,
        input: &Value,
        details: &Value,
        selected: &Value,
        snapshots: (SourceSnapshot, SourceSnapshot),
        confirmation: &'static str,
        now: u64,
    ) -> Result<Value, ApiError> {
        self.expire(now);
        if input["metadata_ref"]["content_ref"]["source_id"] != snapshots.0.source_id
            || input["metadata_ref"]["content_ref"]["source_revision"]
                != snapshots.0.source_revision
            || details["ref"]["source_id"] != snapshots.1.source_id
            || details["ref"]["source_revision"] != snapshots.1.source_revision
            || input["playback_source"]["source_id"] != snapshots.1.source_id
            || input["playback_source"]["source_revision"] != snapshots.1.source_revision
            || snapshots.0.host_epoch != snapshots.1.host_epoch
        {
            return Err(ApiError::bad(
                "Match identities do not belong to captured sources",
            ));
        }
        let episode = selection(details, selected)?;
        if episode["ref"]["source_id"] != snapshots.1.source_id
            || episode["ref"]["source_revision"] != snapshots.1.source_revision
        {
            return Err(ApiError::bad(
                "Selected episode belongs to another provider revision",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let context = json!({"metadata_ref":input["metadata_ref"], "metadata_video_id":input["video_id"], "match_id":id,
            "content_ref":details["ref"], "group_id":selected["group_id"], "episode_id":episode["id"], "episode_ref":episode["ref"], "selected_stream_id":null});
        Contracts::get().validate("PlaybackContext", &context)?;
        if self.records.len() >= MAX_RECORDS {
            let oldest = self
                .records
                .iter()
                .min_by_key(|(_, r)| r.touched)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                self.records.remove(&key);
            }
        }
        let group = details["groups"]
            .as_array()
            .expect("Validated details")
            .iter()
            .find(|g| g["id"] == selected["group_id"])
            .expect("Selected group");
        let label = format!(
            "{} · {}",
            group["title"].as_str().unwrap_or(""),
            episode["label"].as_str().unwrap_or("")
        );
        self.records.insert(
            id,
            MatchRecord {
                context: context.clone(),
                metadata: snapshots.0,
                provider: snapshots.1,
                confirmation,
                label,
                touched: now,
            },
        );
        Ok(context)
    }
    pub fn get(&mut self, id: &str, reference: &Value, now: u64) -> Result<MatchRecord, ApiError> {
        self.expire(now);
        let record = self.records.get_mut(id).ok_or_else(|| {
            ApiError::new(
                "SESSION_EXPIRED",
                "Playback match expired; match this video again",
            )
        })?;
        if record.context["episode_ref"] != *reference {
            return Err(ApiError::bad(
                "Episode does not belong to the confirmed match",
            ));
        }
        record.touched = now;
        Ok(record.clone())
    }
}

#[cfg(test)]
#[path = "../../tests/fixtures/t08_matching.rs"]
mod tests;
