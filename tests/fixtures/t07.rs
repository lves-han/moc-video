//! Synthetic load/DTO boundary only. Real Cloudstream MainAPI execution is N02.
use super::*;
use crate::{
    requests::{MonotonicClock, Requests},
    source_dispatch::{SourceDispatch, identity},
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn registry() -> (SourceDispatch, SourceSnapshot) {
    let mut registry = SourceDispatch::default();
    let config = json!({"kind":"maccms", "config_id":"33333333-3333-4333-8333-333333333333", "enabled":true, "name":"Synthetic guest identity", "api_url":"https://fixture.invalid/api"});
    registry.replace_configs(&[config]);
    let mut captured = registry
        .snapshot(
            "33333333-3333-4333-8333-333333333333",
            "synthetic-host",
            false,
        )
        .unwrap();
    captured.guest_epoch = Some("synthetic-guest".into());
    (registry, captured)
}
fn reference(captured: &SourceSnapshot, data: &str) -> Value {
    json!({"source_id":captured.source_id, "source_revision":captured.source_revision, "data":data})
}
fn image(id: &str) -> Value {
    json!({"resource_id":id, "path":format!("/images/{id}"), "capability":"a".repeat(43), "expires_at":"2030-01-01T00:00:00Z"})
}
fn details(reference: &Value, kind: &str) -> Value {
    let mut groups = Vec::new();
    for (ordinal, axis, season, dub) in [
        (0, "season", Some(0), "unspecified"),
        (1, "route", None, "subbed"),
        (2, "dub", Some(2), "dubbed"),
    ] {
        let group_id = identity("group", &[&ordinal.to_string(), axis]);
        let count = if ordinal == 0 { 200 } else { 2 };
        let episodes: Vec<_> = (0..count).map(|index| {
            let number = if index == 0 { 10 } else if index == 1 { 2 } else { index + 1 };
            let data = if index < 2 { " { \"episode\": \"相同data/%2F\", \"n\":10 }\n".to_owned() } else { format!("https://fixture.invalid/集/{index}?x=%252F&y=$") };
            json!({"id":identity("episode", &[&group_id, &index.to_string(), &data]), "label":format!("第{number}集 / 特别篇 😀"),
                "index":index, "season":season, "number":number, "ref":{"source_id":reference["source_id"], "source_revision":reference["source_revision"], "data":data},
                "poster":if ordinal == 0 && index == 0 { image("episode") } else {Value::Null}, "duration_ms":null, "released_at":null})
        }).collect();
        groups.push(json!({"id":group_id, "title":format!("分组{ordinal} 中文/配音"), "kind":axis, "season_number":season, "dub_status":dub, "language":"zh-CN", "episodes":episodes}));
    }
    json!({"id":identity("content", &[reference["source_id"].as_str().unwrap(), reference["data"].as_str().unwrap()]), "ref":reference,
        "title":"中文 & <特别> 😀", "kind":kind, "plot":null, "year":null, "rating":{"value":0,"scale":10}, "status":"ongoing", "update_label":"更新至第2集", "episode_count":null,
        "poster":image("poster"), "backdrop":image("backdrop"), "tags":[], "regions":[], "languages":[], "duration_ms":120000,
        "actors":[{"name":"演员甲", "role":"主角", "image":image("actor")}], "directors":[{"name":"导演乙", "image":image("director")}], "groups":groups,
        "related":[{"id":"related", "ref":reference, "title":"相关影片", "kind":"movie", "poster":image("related")}]})
}

#[test]
fn t07_guest_original_opaque_movie_series_anime_groups_and_images() {
    let (registry, captured) = registry();
    let requests = Arc::new(Requests::default());
    for kind in ["movie", "series", "anime"] {
        for data in [
            "https://fixture.invalid/中文?x=%2F&y=$#原样",
            " { \"url\":\"https:\\/\\/fixture.invalid/中文\", \"order\":[10,2] }\n",
        ] {
            let reference = reference(&captured, data);
            let expected = details(&reference, kind);
            let job = requests
                .begin(
                    &Uuid::new_v4().to_string(),
                    "default_metadata",
                    Arc::new(MonotonicClock::default()),
                )
                .unwrap();
            let actual = load(
                &captured,
                &reference,
                &job,
                || {
                    registry.check_snapshot(
                        &captured,
                        "synthetic-host",
                        Some("synthetic-guest"),
                        false,
                    )
                },
                |forwarded, child| {
                    assert_eq!(forwarded.as_bytes(), data.as_bytes());
                    assert_eq!(child.budget.total_ms, 30000);
                    Ok(expected.clone())
                },
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(
                actual["groups"][0]["episodes"].as_array().unwrap().len(),
                200
            );
            assert_ne!(
                actual["groups"][0]["episodes"][0]["id"],
                actual["groups"][0]["episodes"][1]["id"]
            );
            assert_eq!(
                actual["groups"][0]["episodes"][0]["ref"],
                actual["groups"][0]["episodes"][1]["ref"]
            );
            let mut registered = std::collections::HashMap::new();
            images(&actual, &mut |image| {
                registered.insert(
                    image["resource_id"].as_str().unwrap().to_owned(),
                    image.clone(),
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(registered.len(), 6);
            for missing in [
                "poster", "backdrop", "episode", "actor", "director", "related",
            ] {
                let mut incomplete = registered.clone();
                incomplete.remove(missing);
                assert!(
                    images(&actual, &mut |image| {
                        if incomplete.get(image["resource_id"].as_str().unwrap()) == Some(image) {
                            Ok(())
                        } else {
                            Err(ApiError::new(
                                "UPSTREAM_FAILED",
                                "Missing synthetic registration",
                            ))
                        }
                    })
                    .is_err()
                );
            }
            println!(
                "T07_VECTOR:{}",
                json!({"adapter":"guest", "synthetic":true, "reference":reference, "expected":expected, "details":actual, "registered":registered})
            );
        }
    }
}

#[test]
fn t07_common_missing_fields_rewritten_refs_and_invalid_relationships() {
    let (_, captured) = registry();
    let reference = reference(&captured, "opaque");
    let valid = details(&reference, "anime");
    validate(&reference, &valid).unwrap();
    for mutation in [
        "missing-groups",
        "missing-index",
        "missing-ref",
        "data",
        "source",
        "revision",
        "episode-source",
        "episode-revision",
        "related-source",
        "indices",
        "duplicate-group",
        "duplicate-episode",
        "season",
        "rating",
        "unit",
        "unsafe-milliseconds",
    ] {
        let mut invalid = valid.clone();
        match mutation {
            "missing-groups" => {
                invalid.as_object_mut().unwrap().remove("groups");
            }
            "missing-index" => {
                invalid["groups"][0]["episodes"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("index");
            }
            "missing-ref" => {
                invalid.as_object_mut().unwrap().remove("ref");
            }
            "data" | "source" | "revision" => {
                invalid["ref"][match mutation {
                    "data" => "data",
                    "source" => "source_id",
                    _ => "source_revision",
                }] = json!("rewritten");
            }
            "episode-source" => {
                invalid["groups"][0]["episodes"][0]["ref"]["source_id"] = json!("foreign")
            }
            "episode-revision" => {
                invalid["groups"][0]["episodes"][0]["ref"]["source_revision"] = json!("stale")
            }
            "related-source" => invalid["related"][0]["ref"]["source_id"] = json!("foreign"),
            "indices" => invalid["groups"][0]["episodes"][0]["index"] = json!(1),
            "duplicate-group" => invalid["groups"][1]["id"] = invalid["groups"][0]["id"].clone(),
            "duplicate-episode" => {
                invalid["groups"][1]["episodes"][0]["id"] =
                    invalid["groups"][0]["episodes"][0]["id"].clone()
            }
            "season" => invalid["groups"][0]["episodes"][0]["season"] = json!(2),
            "rating" => invalid["rating"]["scale"] = json!(100),
            "unit" => invalid["duration_ms"] = json!(0.5),
            "unsafe-milliseconds" => invalid["duration_ms"] = json!(9_007_199_254_740_992u64),
            _ => unreachable!(),
        }
        assert!(validate(&reference, &invalid).is_err(), "{mutation}");
    }
    let mut empty = valid.clone();
    empty["groups"] = json!([]);
    empty["poster"] = Value::Null;
    empty["backdrop"] = Value::Null;
    empty["rating"] = Value::Null;
    empty["related"] = json!([]);
    empty["actors"] = json!([]);
    empty["directors"] = json!([]);
    validate(&reference, &empty).unwrap();
    println!(
        "T07_VECTOR:{}",
        json!({"adapter":"guest-empty", "synthetic":true, "reference":reference, "expected":empty, "details":empty, "registered":{}})
    );
}

#[test]
fn t07_guest_late_epoch_revision_cancel_timeout_and_error() {
    let (registry, captured) = registry();
    let registry = Mutex::new(registry);
    let reference = reference(&captured, " {\"opaque\":\"原样\"} ");
    let value = details(&reference, "series");
    for mutation in [
        "host",
        "guest",
        "revision",
        "generation",
        "cancel",
        "timeout",
        "error",
    ] {
        let requests = Arc::new(Requests::default());
        let clock = Arc::new(TestClock::default());
        let id = Uuid::new_v4().to_string();
        let job = requests
            .begin(&id, "default_metadata", clock.clone())
            .unwrap();
        let host_epoch = Mutex::new("synthetic-host".to_owned());
        let guest_epoch = Mutex::new("synthetic-guest".to_owned());
        let mut snapshot = captured.clone();
        if mutation == "revision" {
            snapshot.source_revision = "stale".into();
        }
        let result = load(
            &snapshot,
            &reference,
            &job,
            || {
                registry.lock().unwrap().check_snapshot(
                    &captured,
                    &host_epoch.lock().unwrap(),
                    Some(&guest_epoch.lock().unwrap()),
                    false,
                )
            },
            |_, _| {
                match mutation {
                    "host" => *host_epoch.lock().unwrap() = "new-host".into(),
                    "guest" => *guest_epoch.lock().unwrap() = "new-guest".into(),
                    "generation" => {
                        registry.lock().unwrap().replace_configs(&[]);
                    }
                    "cancel" => {
                        requests.cancel(&Uuid::new_v4().to_string(), &id).unwrap();
                    }
                    "timeout" => clock.0.store(30000, std::sync::atomic::Ordering::SeqCst),
                    "error" => {
                        return Err(ApiError::new("UPSTREAM_FAILED", "Synthetic load failure"));
                    }
                    _ => unreachable!(),
                }
                Ok(value.clone())
            },
        );
        assert_eq!(
            result.unwrap_err().code,
            match mutation {
                "revision" => "REVISION_MISMATCH",
                "timeout" => "UPSTREAM_TIMEOUT",
                "error" => "UPSTREAM_FAILED",
                _ => "CANCELLED",
            }
        );
        // Restore the registry for the next independent negative.
        if mutation == "generation" {
            *registry.lock().unwrap() = self::registry().0;
        }
        drop(job);
        assert_eq!(requests.active_count(), 0);
    }
}

#[derive(Default)]
struct TestClock(std::sync::atomic::AtomicU64);
impl crate::requests::Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}
