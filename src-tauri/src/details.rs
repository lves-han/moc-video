//! Common detail relationships. Provider data and array order remain opaque.
use crate::{
    contracts::{ApiError, Contracts, wire_size},
    requests::Job,
    source_dispatch::SourceSnapshot,
};
use serde_json::{Value, json};
use std::collections::HashSet;

pub fn validate(reference: &Value, details: &Value) -> Result<(), ApiError> {
    let bad = || {
        ApiError::new(
            "UPSTREAM_FAILED",
            "Details violate reference ownership or episode ordering",
        )
    };
    wire_size(details, 8 * 1024 * 1024)?;
    Contracts::get()
        .validate(
            "DetailsViewData",
            &json!({"details": details, "images": {}, "metadata_ref": null}),
        )
        .map_err(|_| bad())?;
    if details["ref"] != *reference {
        return Err(bad());
    }
    let owned = |r: &Value| {
        r["source_id"] == reference["source_id"]
            && r["source_revision"] == reference["source_revision"]
    };
    let mut groups = HashSet::new();
    let mut episodes = HashSet::new();
    for group in details["groups"].as_array().expect("Validated groups") {
        if !groups.insert(group["id"].as_str().expect("Validated group ID")) {
            return Err(bad());
        }
        for (index, episode) in group["episodes"]
            .as_array()
            .expect("Validated episodes")
            .iter()
            .enumerate()
        {
            if episode["index"] != index
                || !owned(&episode["ref"])
                || !episodes.insert(episode["id"].as_str().expect("Validated episode ID"))
                || (group["kind"] == "season"
                    && !group["season_number"].is_null()
                    && episode["season"] != group["season_number"])
            {
                return Err(bad());
            }
        }
    }
    if details["related"]
        .as_array()
        .expect("Validated related cards")
        .iter()
        .any(|card| !owned(&card["ref"]))
    {
        return Err(bad());
    }
    Ok(())
}

/// Visit every image, including related cards, people and episode thumbnails.
/// The callback stages a registration or verifies an existing native record.
pub fn images(
    value: &Value,
    register: &mut impl FnMut(&Value) -> Result<(), ApiError>,
) -> Result<(), ApiError> {
    match value {
        Value::Object(fields)
            if fields.contains_key("resource_id") && fields.contains_key("capability") =>
        {
            register(value)?
        }
        Value::Object(fields) => {
            for child in fields.values() {
                images(child, register)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                images(child, register)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Bridge load boundary; the real MainAPI transport is connected in N02.
/// The caller checks its captured host/guest/config snapshot under its commit
/// lock before and after load, so a late response cannot publish registrations.
pub fn load(
    captured: &SourceSnapshot,
    reference: &Value,
    job: &Job,
    check_current: impl Fn() -> Result<(), ApiError>,
    fetch: impl FnOnce(&str, &Job) -> Result<Value, ApiError>,
) -> Result<Value, ApiError> {
    check_current()?;
    if reference["source_id"] != captured.source_id
        || reference["source_revision"] != captured.source_revision
    {
        return Err(ApiError::new(
            "REVISION_MISMATCH",
            "Details reference belongs to another source revision",
        ));
    }
    let data = reference["data"]
        .as_str()
        .ok_or_else(|| ApiError::bad("Details require opaque data"))?;
    wire_size(reference, 1024 * 1024)?;
    job.check()?;
    let details = fetch(data, job)?;
    job.check()?;
    check_current()?;
    validate(reference, &details)?;
    Ok(details)
}

#[cfg(test)]
#[path = "../../tests/fixtures/t07.rs"]
mod tests;
