//! Synthetic original-loadLinks interface and reference-ownership vectors.
use super::*;
use crate::requests::{Clock, Requests};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn job(clock: Arc<TestClock>) -> Job {
    Arc::new(Requests::default())
        .begin(&Uuid::new_v4().to_string(), "resolve", clock)
        .unwrap()
}
fn reference() -> Value {
    json!({"source_id":"synthetic-provider","source_revision":"revision1","data":" { \"data\": \"中文/%2F/$\" }\n"})
}
fn link(label: &str) -> Link {
    let mut headers = HeaderMap::new();
    headers.insert("x-original", "header-value".parse().unwrap());
    Link {
        original: json!({"name":label,"url":"https://media.invalid/original?x=%2F&y=$","data":"extractor bytes"}),
        provider: "synthetic-provider".into(),
        url: "https://media.invalid/original?x=%2F&y=$".into(),
        headers,
        referer: "https://provider.invalid/ref?raw=%2F".into(),
        extractor_data: Some(" { opaque data }\n".into()),
        interceptor: None,
        cookies: None,
        label: label.into(),
        quality: None,
        quality_label: Some("Unknown".into()),
        kind: "hls".into(),
        mime_type: Some("application/vnd.apple.mpegurl".into()),
        playable: true,
        reason: None,
    }
}
fn subtitle() -> Subtitle {
    Subtitle {
        original: json!({"url":"https://media.invalid/caption.ass","lang":"zh-CN"}),
        url: "https://media.invalid/caption.ass".into(),
        headers: HeaderMap::new(),
        label: "中文原字幕".into(),
        language: Some("zh-CN".into()),
        format: "ass".into(),
    }
}

#[test]
fn t08_loadlinks_multiple_subtitles_false_unknown_quality_and_original_data() {
    for returned in [true, false] {
        let clock = Arc::new(TestClock::default());
        let reference = reference();
        let job = job(clock);
        let late = Arc::new(Mutex::new(None));
        let capture = late.clone();
        let (links, subtitles) = load_links(
            &reference,
            &job,
            || Ok(()),
            |data, is_casting, callbacks, child| {
                assert_eq!(
                    data.as_bytes(),
                    reference["data"].as_str().unwrap().as_bytes()
                );
                assert!(!is_casting);
                assert_eq!(child.child_timeout_ms().unwrap(), 45000);
                callbacks.link(link("first 原始"))?;
                callbacks.subtitle(subtitle())?;
                callbacks.link(link("second 同名"))?;
                *capture.lock().unwrap() = Some(callbacks);
                Ok(returned)
            },
        )
        .unwrap();
        assert_eq!(
            late.lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .link(link("late"))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        let mut store = ResolutionStore::default();
        let resolution = store.register(&reference, links, subtitles, 0).unwrap();
        assert_eq!(resolution["episode_ref"], reference);
        assert_eq!(resolution["streams"][0]["label"], "first 原始");
        assert_eq!(resolution["streams"][1]["label"], "second 同名");
        assert!(resolution["streams"][0]["quality"].is_null());
        assert_eq!(resolution["subtitles"][0]["format"], "ass");
        let record = store
            .get(
                resolution["id"].as_str().unwrap(),
                resolution["streams"][0]["id"].as_str().unwrap(),
                "revision1",
                0,
            )
            .unwrap();
        assert_eq!(record.link.headers["x-original"], "header-value");
        assert_eq!(record.link.referer, "https://provider.invalid/ref?raw=%2F");
        assert_eq!(
            record.link.extractor_data.as_deref(),
            Some(" { opaque data }\n")
        );
        assert_eq!(record.subtitles[0].subtitle.original, subtitle().original);
        assert_eq!(record.subtitles[0].id, resolution["subtitles"][0]["id"]);
        assert_eq!(record.stream_id, resolution["streams"][0]["id"]);
        assert_eq!(record.resolution_id, resolution["id"]);
        assert_eq!(record.link.original, link("first 原始").original);
        println!(
            "T08_VECTOR:{}",
            json!({"kind":"resolution","synthetic":true,"returned":returned,"reference":reference,"resolution":resolution})
        );
    }
}

#[test]
fn t08_empty_false_exception_late_cancel_timeout_and_revision() {
    let clock = Arc::new(TestClock::default());
    for returned in [true, false] {
        let error = load_links(
            &reference(),
            &job(clock.clone()),
            || Ok(()),
            |_, _, callbacks, _| {
                callbacks.subtitle(subtitle())?;
                Ok(returned)
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "UPSTREAM_FAILED");
        assert_eq!(error.details.unwrap()["reason"], "no_links");
    }
    let callbacks = Arc::new(Mutex::new(None));
    let saved = callbacks.clone();
    let error = load_links(
        &reference(),
        &job(clock.clone()),
        || Ok(()),
        |_, _, cb, _| {
            cb.link(link("discard"))?;
            *saved.lock().unwrap() = Some(cb);
            Err(ApiError::new("UPSTREAM_FAILED", "provider exception"))
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.message, "provider exception");
    assert!(
        callbacks
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .subtitle(subtitle())
            .is_err()
    );
    let requests = Arc::new(Requests::default());
    let id = Uuid::new_v4().to_string();
    let request = requests.begin(&id, "resolve", clock.clone()).unwrap();
    let error = load_links(
        &reference(),
        &request,
        || Ok(()),
        |_, _, cb, _| {
            cb.link(link("cancel"))?;
            requests.cancel(&Uuid::new_v4().to_string(), &id)?;
            Ok(true)
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "CANCELLED");
    let timed = job(clock.clone());
    let error = load_links(
        &reference(),
        &timed,
        || Ok(()),
        |_, _, cb, _| {
            cb.link(link("late"))?;
            clock.0.store(45000, Ordering::SeqCst);
            Ok(true)
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "UPSTREAM_TIMEOUT");
    let current = AtomicUsize::new(0);
    let error = load_links(
        &reference(),
        &job(Arc::new(TestClock::default())),
        || {
            if current.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err(ApiError::cancelled())
            }
        },
        |_, _, cb, _| {
            cb.link(link("revision changed"))?;
            Ok(true)
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.code, "CANCELLED");
}

#[test]
fn t08_unplayable_empty_links_provider_and_revision_isolation() {
    let mut store = ResolutionStore::default();
    let mut empty = link("empty callback");
    empty.url.clear();
    let mut drm = link("DRM");
    drm.kind = "drm".into();
    drm.reason = Some("drm".into());
    let result = store
        .register(&reference(), vec![empty, drm], vec![], 0)
        .unwrap();
    assert!(
        result["streams"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["playable"] == false && s["reason"].as_str().is_some_and(|s| !s.is_empty()))
    );
    let id = result["id"].as_str().unwrap();
    let stream = result["streams"][0]["id"].as_str().unwrap();
    assert_eq!(
        store.get(id, stream, "revision2", 0).err().unwrap().code,
        "SESSION_EXPIRED"
    );
    assert!(store.get(id, "foreign", "revision1", 0).is_err());
    let mut wrong = link("wrong provider");
    wrong.provider = "foreign".into();
    assert!(
        store
            .register(&reference(), vec![wrong], vec![], 0)
            .is_err()
    );
    assert_eq!(store.records.len(), 1);
    assert!(store.get(id, stream, "revision1", 299999).is_ok());
    assert!(store.get(id, stream, "revision1", 300000).is_err());
}

struct Owned(Arc<AtomicUsize>);
impl Drop for Owned {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn t08_release_retained_network_context_and_active_request_ownership() {
    for action in ["release", "expiry", "revision"] {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut original = link("owned");
        original.interceptor = Some(Arc::new(Owned(drops.clone())));
        original.cookies = Some(Arc::new(Owned(drops.clone())));
        let mut store = ResolutionStore::default();
        let value = store
            .register(&reference(), vec![original], vec![subtitle()], 0)
            .unwrap();
        let id = value["id"].as_str().unwrap();
        let stream = value["streams"][0]["id"].as_str().unwrap();
        let session = store.get(id, stream, "revision1", 0).unwrap();
        let active = session.clone();
        assert!(value["streams"][0]["has_interceptor"].as_bool().unwrap());
        match action {
            "release" => {
                store.release(id);
                store.release(id);
            }
            "expiry" => store.expire(300000),
            _ => store.revoke(&["synthetic-provider".into()]),
        }
        assert_eq!(
            store.get(id, stream, "revision1", 0).err().unwrap().code,
            "SESSION_EXPIRED"
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(session.link.original, link("owned").original);
        assert_eq!(session.subtitles.len(), 1);
        drop(session);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(active);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }
}
