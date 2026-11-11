//! Shared home DTO rules. Cloudstream transport/provider execution is supplied
//! by the bridge; this boundary keeps its opaque section references intact.
use crate::{
    contracts::{ApiError, Contracts, wire_size},
    requests::Job,
    source_dispatch::identity,
};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Mutex, time::Duration};

pub fn unavailable_filters() -> Value {
    let mut filters = json!({});
    for key in ["genre", "year", "language", "region", "sort"] {
        filters[key] = json!({"available":false,"options":[]});
    }
    filters
}

pub fn operation(kind: &str, input: &Value) -> String {
    if input["page"] == 1 && input["section_ref"].is_null() {
        format!("{kind}_home")
    } else if kind == "cloudstream" {
        "cloudstream_home_page".into()
    } else {
        "default_metadata".into()
    }
}

pub fn reserve_error(job: &Job) -> Result<Option<ApiError>, ApiError> {
    Ok((job.aggregate_remaining_ms()? == 0).then(|| {
        ApiError::new(
            "UPSTREAM_TIMEOUT",
            "Home reached its result cleanup reserve",
        )
    }))
}

/// A column's redirects, localization and fallback share one child deadline.
/// The parent stays alive for serialization after stopping remaining children.
pub fn column<T: Send>(
    job: &Job,
    aggregate: bool,
    call: impl FnOnce(&Job) -> Result<T, ApiError> + Send,
) -> Result<T, ApiError> {
    if !aggregate {
        return call(job);
    }
    if let Some(error) = reserve_error(job)? {
        return Err(error);
    }
    let child = job.child(&uuid::Uuid::new_v4().to_string(), "default_metadata")?;
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(move || call(&child));
        while !worker.is_finished() {
            if job.aggregate_remaining_ms().is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        worker
            .join()
            .unwrap_or_else(|_| Err(ApiError::internal("Home column worker failed")))
    });
    job.check()?;
    match result {
        Err(error) if error.code == "CANCELLED" && reserve_error(job)?.is_some() => Err(
            ApiError::new("UPSTREAM_TIMEOUT", "Home column stopped for result cleanup"),
        ),
        result => result,
    }
}

pub fn failure(title: &str, handle: &str, index: usize, error: &ApiError) -> Value {
    json!({"title": title, "section_ref": handle, "index": index, "error": error})
}

pub fn page(
    input: &Value,
    sections: Vec<Value>,
    failed: Vec<Value>,
    first_error: Option<ApiError>,
) -> Result<Value, ApiError> {
    if sections.is_empty() && !failed.is_empty() {
        return Err(first_error.unwrap_or_else(|| {
            ApiError::new("UPSTREAM_FAILED", "Home has no successful sections")
        }));
    }
    let page = json!({"source_id": input["source_id"], "source_revision": input["source_revision"],
        "sections": sections, "failed_sections": failed});
    validate(input, &page)?;
    Ok(page)
}

/// Schema validation covers fields; these checks cover request scope and the
/// relationships used by retries, ordering and pagination.
pub fn validate(input: &Value, page: &Value) -> Result<(), ApiError> {
    let bad = || {
        ApiError::new(
            "UPSTREAM_FAILED",
            "Home response violates request scope or section ordering",
        )
    };
    wire_size(page, 8 * 1024 * 1024)?;
    Contracts::get()
        .validate("HomeViewData", &json!({"page": page, "images": {}}))
        .map_err(|_| bad())?;
    if page["source_id"] != input["source_id"]
        || page["source_revision"] != input["source_revision"]
    {
        return Err(bad());
    }
    let requested_page = input["page"]
        .as_u64()
        .filter(|p| (1..=10000).contains(p))
        .ok_or_else(|| ApiError::bad("Home page must be between 1 and 10000"))?;
    let handle = input["section_ref"].as_str();
    if handle.is_none() && requested_page != 1 {
        return Err(ApiError::bad("Full home requires page 1"));
    }
    let sections = page["sections"].as_array().expect("Validated sections");
    let failed = page["failed_sections"]
        .as_array()
        .expect("Validated failures");
    if sections.is_empty() && !failed.is_empty() {
        return Err(bad());
    }
    if handle.is_some() && (sections.len() != 1 || !failed.is_empty()) {
        return Err(bad());
    }
    let mut indices = HashSet::new();
    let mut handles = HashSet::new();
    let mut ids = HashSet::new();
    for rows in [sections, failed] {
        let mut previous = None;
        for row in rows {
            let index = row["index"].as_u64().expect("Validated index");
            if !indices.insert(index) || previous.is_some_and(|p| p >= index) {
                return Err(bad());
            }
            previous = Some(index);
            if let Some(reference) = row["section_ref"].as_str() {
                if !handles.insert(reference) || handle.is_some_and(|h| h != reference) {
                    return Err(bad());
                }
            } else {
                return Err(bad());
            }
        }
    }
    if handle.is_none() && (0..indices.len() as u64).any(|index| !indices.contains(&index)) {
        return Err(bad());
    }
    for section in sections {
        if !ids.insert(section["id"].as_str().expect("Validated section ID"))
            || section["page"] != requested_page
            || (section["has_next"] == true
                && (requested_page == 10000 || section["next_page"] != requested_page + 1))
            || (section["has_next"] == false && !section["next_page"].is_null())
        {
            return Err(bad());
        }
        for card in section["items"].as_array().expect("Validated items") {
            if card["ref"]["source_id"] != input["source_id"]
                || card["ref"]["source_revision"] != input["source_revision"]
            {
                return Err(bad());
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Binding {
    host: String,
    guest: String,
    id: Option<String>,
    index: u64,
    next_page: Option<u64>,
}

/// Per-provider bridge DTO adapter. A bridge response can contain dynamic and
/// multiple sections from one getMainPage call. Their guest references retain
/// the original MainPageRequest; the host never reconstructs it from a title.
pub struct GuestHome {
    host_epoch: String,
    guest_epoch: String,
    source_id: String,
    revision: String,
    bindings: Mutex<Vec<Binding>>,
}

impl GuestHome {
    pub fn new(
        host_epoch: String,
        guest_epoch: String,
        source_id: String,
        revision: String,
    ) -> Self {
        Self {
            host_epoch,
            guest_epoch,
            source_id,
            revision,
            bindings: Mutex::new(Vec::new()),
        }
    }

    pub fn request(
        &self,
        input: &Value,
        host_epoch: &str,
        guest_epoch: &str,
        job: &Job,
        fetch: impl FnOnce(&Value, &Job) -> Result<Value, ApiError>,
    ) -> Result<Value, ApiError> {
        if host_epoch != self.host_epoch || guest_epoch != self.guest_epoch {
            return Err(ApiError::cancelled());
        }
        if input["source_id"] != self.source_id || input["source_revision"] != self.revision {
            return Err(ApiError::new(
                "REVISION_MISMATCH",
                "Home reference belongs to another source revision",
            ));
        }
        if job.budget != crate::requests::budgets()[&operation("cloudstream", input)] {
            return Err(ApiError::bad("Home job has the wrong operation budget"));
        }
        // One metadata operation per provider, including sequential providers.
        // Waiting consumes the admitted deadline and remains cancellable.
        let mut bindings = loop {
            job.check()?;
            match self.bindings.try_lock() {
                Ok(bindings) => break bindings,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(_) => return Err(ApiError::internal("Home reference registry lock poisoned")),
            }
        };
        let mut guest_input = input.clone();
        let selected = if let Some(handle) = input["section_ref"].as_str() {
            let record = bindings
                .iter()
                .find(|b| b.host == handle)
                .ok_or_else(|| ApiError::bad("Section reference is not registered for this epoch"))?
                .clone();
            if input["page"] != 1 && input["page"].as_u64() != record.next_page {
                return Err(ApiError::bad("Section page has no registered continuation"));
            }
            guest_input["section_ref"] = json!(record.guest);
            Some(record)
        } else {
            if input["page"] != 1 {
                return Err(ApiError::bad("Full home requires page 1"));
            }
            None
        };
        if selected.is_none()
            && let Some(error) = reserve_error(job)?
        {
            return Err(error);
        }
        let mut page = fetch(&guest_input, job)?;
        job.check()?;
        validate(&guest_input, &page)?;
        let mut updated = if selected.is_some() {
            bindings.clone()
        } else {
            Vec::new()
        };
        for field in ["sections", "failed_sections"] {
            for section in page[field].as_array_mut().expect("Validated sections") {
                let guest = section["section_ref"]
                    .as_str()
                    .expect("Validated handle")
                    .to_owned();
                let index = section["index"].as_u64().expect("Validated index");
                let id = section["id"].as_str().map(str::to_owned);
                if selected.as_ref().is_some_and(|record| {
                    record.index != index
                        || record
                            .id
                            .as_ref()
                            .is_some_and(|old| Some(old) != id.as_ref())
                }) {
                    return Err(ApiError::new(
                        "UPSTREAM_FAILED",
                        "Bridge changed the registered section identity",
                    ));
                }
                let host = identity(
                    "host_section_ref",
                    &[
                        &self.host_epoch,
                        &self.guest_epoch,
                        &self.source_id,
                        &self.revision,
                        &guest,
                    ],
                );
                let binding = Binding {
                    host: host.clone(),
                    guest,
                    id,
                    index,
                    next_page: section["next_page"].as_u64(),
                };
                if let Some(existing) = updated.iter_mut().find(|b| b.host == host) {
                    *existing = binding;
                } else {
                    updated.push(binding);
                }
                section["section_ref"] = json!(host);
            }
        }
        if updated.len() > 4096 {
            return Err(ApiError::new(
                "RESOURCE_LIMIT",
                "Home section registry exceeds 4096 records",
            ));
        }
        validate(input, &page)?;
        job.check()?;
        *bindings = updated;
        Ok(page)
    }
}

#[cfg(test)]
#[path = "../../tests/fixtures/t06.rs"]
mod tests;
