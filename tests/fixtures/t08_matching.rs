//! Synthetic provider evidence; HTTP and application integration are separate.
use super::*;
use crate::{
    requests::{Clock, Requests},
    source_dispatch::SourceDispatch,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn job(clock: Arc<TestClock>) -> Job {
    Arc::new(Requests::default())
        .begin(&Uuid::new_v4().to_string(), "match_playback", clock)
        .unwrap()
}
fn snapshot(config: &str) -> SourceSnapshot {
    let mut sources = SourceDispatch::default();
    sources.replace_configs(&[json!({"kind":"maccms", "config_id":config, "enabled":true, "name":"fixture", "api_url":"https://fixture.invalid/api"})]);
    sources.snapshot(config, "synthetic-host", false).unwrap()
}
fn vectors() -> (Value, Value, Value, SourceSnapshot, SourceSnapshot) {
    let meta = snapshot("11111111-1111-4111-8111-111111111111");
    let provider = snapshot("22222222-2222-4222-8222-222222222222");
    let reference = json!({"source_id":provider.source_id, "source_revision":provider.source_revision, "data":"original content 😀"});
    let details = json!({"ref":reference, "title":"Same title", "year":null, "kind":"series", "groups":[{"id":"group0", "title":"Original route", "episodes":[{"id":"ep0", "label":"Special 0.5", "season":0, "number":0.5, "ref":{"source_id":provider.source_id, "source_revision":provider.source_revision, "data":" { \"original\": \"%2F/$/中文\" }\n"}}]}]});
    let input = json!({"metadata_ref":{"identity":{"config_id":meta.config["config_id"], "addon_key":"a".repeat(64), "namespace":"imdb", "type":"series", "meta_id":"tt12345", "canonical_id":"tt12345"}, "content_ref":{"source_id":meta.source_id,"source_revision":meta.source_revision,"data":"metadata"}}, "video_id":"raw:0:0.5", "playback_source":{"source_id":provider.source_id,"source_revision":provider.source_revision}, "selection":null});
    let metadata = json!({"metadata_ref":input["metadata_ref"], "details":{"title":"Same title", "groups":[{"episodes":[{"season":0,"number":0.5,"ref":{"data":json!(["stremio_video","addon","series","tt12345","raw:0:0.5"]).to_string()}}]}]}});
    (input, metadata, details, meta, provider)
}
fn candidate_for(details: &Value, verified: bool) -> Candidate {
    Candidate {
        details: details.clone(),
        external_identity: verified.then(|| ("imdb".into(), "tt12345".into(), "series".into())),
        external_videos: HashMap::new(),
    }
}

#[test]
fn t08_verified_id_unique_special_decimal_and_ambiguities() {
    let (input, metadata, details, _, _) = vectors();
    let clock = Arc::new(TestClock::default());
    let requested = video(&metadata, input["video_id"].as_str().unwrap()).unwrap();
    let run = |items: Vec<Candidate>| {
        search(&metadata, &requested, &[], &job(clock.clone()), |_, _| {
            Ok(items.clone())
        })
        .unwrap()
    };
    let exact = run(vec![candidate_for(&details, true)]);
    assert!(exact.selected.is_some());
    for mode in [
        "title_only",
        "remake",
        "alias",
        "unknown_year",
        "wrong_type",
        "wrong_id",
        "season",
        "decimal",
        "unknown_season",
        "duplicate_group",
        "two_contents",
    ] {
        let mut item = candidate_for(&details, true);
        match mode {
            "title_only" | "remake" | "alias" | "unknown_year" => {
                item.external_identity = None;
                item.details["title"] = json!(mode);
                item.details["year"] = json!(if mode == "unknown_year" {
                    None
                } else {
                    Some(2024)
                });
            }
            "wrong_type" => item.details["kind"] = json!("movie"),
            "wrong_id" => item.external_identity.as_mut().unwrap().1 = "tt99999".into(),
            "season" => item.details["groups"][0]["episodes"][0]["season"] = json!(1),
            "decimal" => item.details["groups"][0]["episodes"][0]["number"] = json!(0),
            "unknown_season" => item.details["groups"][0]["episodes"][0]["season"] = Value::Null,
            "duplicate_group" => {
                let mut group = item.details["groups"][0].clone();
                group["id"] = json!("dub");
                item.details["groups"].as_array_mut().unwrap().push(group);
            }
            _ => {}
        }
        let mut items = vec![item];
        if mode == "two_contents" {
            let mut second = candidate_for(&details, true);
            second.details["ref"]["data"] = json!("another version");
            items.push(second);
        }
        assert!(run(items).selected.is_none(), "{mode} auto-selected");
    }
    let mut unknown = requested.clone();
    unknown["season"] = Value::Null;
    unknown["number"] = Value::Null;
    assert!(
        search(&metadata, &unknown, &[], &job(clock.clone()), |_, _| Ok(
            vec![candidate_for(&details, true)]
        ))
        .unwrap()
        .selected
        .is_none()
    );
    let mut explicit = candidate_for(&details, true);
    explicit
        .external_videos
        .insert("ep0".into(), "raw:0:0.5".into());
    assert!(
        search(&metadata, &unknown, &[], &job(clock.clone()), |_, _| Ok(
            vec![explicit.clone()]
        ))
        .unwrap()
        .selected
        .is_some()
    );
    explicit
        .external_videos
        .insert("ep0".into(), "wrong-video".into());
    assert!(
        search(&metadata, &unknown, &[], &job(clock.clone()), |_, _| Ok(
            vec![explicit.clone()]
        ))
        .unwrap()
        .selected
        .is_none()
    );
    let mut movie_metadata = metadata.clone();
    movie_metadata["metadata_ref"]["identity"]["type"] = json!("movie");
    let mut movie = candidate_for(&details, true);
    movie.details["kind"] = json!("movie");
    movie.external_identity.as_mut().unwrap().2 = "movie".into();
    assert!(
        search(
            &movie_metadata,
            &unknown,
            &[],
            &job(clock.clone()),
            |_, _| Ok(vec![movie.clone()])
        )
        .unwrap()
        .selected
        .is_some()
    );
    let mut across = candidate_for(&details, true);
    let mut other = across.details["groups"][0].clone();
    other["id"] = json!("season2");
    other["episodes"][0]["season"] = json!(2);
    across.details["groups"].as_array_mut().unwrap().push(other);
    assert!(run(vec![across]).selected.is_some());
    println!(
        "T08_VECTOR:{}",
        json!({"kind":"matching", "synthetic":true, "metadata":metadata, "provider":details, "selected":exact.selected.unwrap().1})
    );
}

#[test]
fn t08_queries_two_ten_twenty_deadline_errors_and_zero() {
    let (_, metadata, details, _, _) = vectors();
    let clock = Arc::new(TestClock::default());
    let mut calls = Vec::new();
    let proposal = search(
        &metadata,
        &json!({}),
        &[" SAME  title ".into(), "已有别名".into(), "third".into()],
        &job(clock.clone()),
        |title, child| {
            calls.push(title.to_owned());
            assert_eq!(child.child_timeout_ms().unwrap(), 15000);
            Ok((0..15)
                .map(|i| {
                    let mut d = details.clone();
                    d["ref"]["data"] = json!(format!("{title}/{i}"));
                    candidate_for(&d, false)
                })
                .collect())
        },
    )
    .unwrap();
    assert_eq!(calls, vec!["Same title", "已有别名"]);
    assert_eq!(proposal.candidates.len(), 20);
    assert_eq!(proposal.candidates[10]["content_ref"]["data"], "已有别名/0");
    let empty = search(&metadata, &json!({}), &[], &job(clock.clone()), |_, _| {
        Ok(vec![])
    })
    .unwrap();
    assert!(empty.candidates.is_empty());
    let err = search(&metadata, &json!({}), &[], &job(clock.clone()), |_, _| {
        Err(ApiError::new("UPSTREAM_FAILED", "fixture"))
    })
    .err()
    .unwrap();
    assert_eq!(err.code, "UPSTREAM_FAILED");
    let timed = job(clock.clone());
    clock.0.store(43000, Ordering::SeqCst);
    let child = timed
        .aggregate_child(&Uuid::new_v4().to_string(), "default_metadata")
        .unwrap();
    assert_eq!(child.remaining_ms().unwrap(), 1000);
    clock.0.store(44000, Ordering::SeqCst);
    assert_eq!(child.check().unwrap_err().code, "UPSTREAM_TIMEOUT");
    let mut invoked = false;
    assert_eq!(
        search(&metadata, &json!({}), &[], &timed, |_, _| {
            invoked = true;
            Ok(vec![])
        })
        .err()
        .unwrap()
        .code,
        "UPSTREAM_TIMEOUT"
    );
    assert!(!invoked);
    clock.0.store(45000, Ordering::SeqCst);
    assert_eq!(timed.check().unwrap_err().code, "UPSTREAM_TIMEOUT");
}

#[test]
fn t08_match_store_binding_selection_lru_idle_revoke_and_order() {
    let (input, metadata, details, meta, provider) = vectors();
    assert!(video(&metadata, "wrong-video").is_err());
    let selected = json!({"content_ref":details["ref"],"group_id":"group0","episode_id":"ep0"});
    let mut store = MatchStore::default();
    assert!(store.registered(&selected, 0).is_err());
    store.register_details(details.clone(), 0);
    assert_eq!(store.registered(&selected, 0).unwrap(), details);
    for field in ["content_ref", "group_id", "episode_id"] {
        let mut invalid = selected.clone();
        if field == "content_ref" {
            invalid[field]["data"] = json!("foreign");
        } else {
            invalid[field] = json!("foreign");
        }
        assert!(store.registered(&invalid, 0).is_err());
    }
    let seq = store.begin(&input);
    let next = store.begin(&input);
    assert_eq!(store.check(seq).unwrap_err().code, "CANCELLED");
    assert!(store.check(next).is_ok());
    let context = store
        .issue(
            &input,
            &details,
            &selected,
            (meta.clone(), provider.clone()),
            "manual",
            0,
        )
        .unwrap();
    let id = context["match_id"].as_str().unwrap();
    let reference = &context["episode_ref"];
    let mut wrong = reference.clone();
    wrong["data"] = json!("wrong");
    assert!(store.get(id, &wrong, 0).is_err());
    assert_eq!(
        store.get(id, reference, 299999).unwrap().confirmation,
        "manual"
    );
    assert!(store.get(id, reference, 599998).is_ok());
    assert_eq!(
        store.get(id, reference, 899998).err().unwrap().code,
        "SESSION_EXPIRED"
    );
    for n in 0..257 {
        store
            .issue(
                &input,
                &details,
                &selected,
                (meta.clone(), provider.clone()),
                "manual",
                n,
            )
            .unwrap();
    }
    assert_eq!(store.records.len(), 256);
    let mut forged = input.clone();
    forged["playback_source"]["source_id"] = json!("foreign");
    assert!(
        store
            .issue(
                &forged,
                &details,
                &selected,
                (meta.clone(), provider.clone()),
                "manual",
                300
            )
            .is_err()
    );
    let mut wrong_epoch = provider.clone();
    wrong_epoch.host_epoch = "other-host".into();
    assert!(
        store
            .issue(
                &input,
                &details,
                &selected,
                (meta.clone(), wrong_epoch),
                "manual",
                300
            )
            .is_err()
    );
    let mut wrong_revision = provider.clone();
    wrong_revision.source_revision = "other-revision".into();
    assert!(
        store
            .issue(
                &input,
                &details,
                &selected,
                (meta.clone(), wrong_revision),
                "manual",
                300
            )
            .is_err()
    );
    store.revoke(std::slice::from_ref(&provider.source_id));
    assert!(store.records.is_empty());
    store
        .issue(
            &input,
            &details,
            &selected,
            (meta.clone(), provider.clone()),
            "manual",
            0,
        )
        .unwrap();
    let mut new_scope = input.clone();
    new_scope["video_id"] = json!("other");
    store.begin(&new_scope);
    assert!(store.records.is_empty());
    println!(
        "T08_VECTOR:{}",
        json!({"kind":"context","synthetic":true,"input":input,"details":details,"context":context})
    );
}

#[test]
fn t08_dual_source_cancellation_and_child_cleanup() {
    let requests = Arc::new(Requests::default());
    let clock = Arc::new(TestClock::default());
    let parent = requests
        .begin(&Uuid::new_v4().to_string(), "match_playback", clock)
        .unwrap();
    parent.bind_source("metadata");
    parent.bind_source("provider");
    let child = parent
        .child(&Uuid::new_v4().to_string(), "default_metadata")
        .unwrap();
    requests.cancel_sources(&["metadata".into()]);
    assert_eq!(child.check().unwrap_err().code, "CANCELLED");
    assert_eq!(parent.check().unwrap_err().code, "CANCELLED");
    drop(child);
    drop(parent);
    assert_eq!(requests.active_count(), 0);
}
