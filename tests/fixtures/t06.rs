//! Synthetic common DTO/guest-boundary vectors. No real Cloudstream provider
//! or Android bridge is exercised here; that gate belongs to N02.
use super::*;
use crate::requests::{Clock, Requests, budgets};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
    mpsc,
};
use uuid::Uuid;

#[derive(Default)]
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../../protocol/examples/contract-cases.json")).unwrap()
}
fn vector() -> Value {
    fixture()["semantic_cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "home_full_response_order")
        .unwrap()
        .clone()
}
fn section(index: usize, reference: &str) -> Value {
    let mut section = vector()["value"]["sections"][0].clone();
    section["id"] = json!(format!("section-{reference}"));
    section["index"] = json!(index);
    section["section_ref"] = json!(reference);
    section["title"] = json!("重复标题 / Synthetic");
    section["items"] = json!([]);
    section
}
fn input() -> Value {
    vector()["request"].clone()
}
fn dto(sections: Vec<Value>, failed: Vec<Value>) -> Value {
    json!({"source_id":input()["source_id"], "source_revision":input()["source_revision"],
        "sections":sections, "failed_sections":failed})
}
fn context() -> GuestHome {
    GuestHome::new(
        "host-epoch".into(),
        "guest-epoch".into(),
        input()["source_id"].as_str().unwrap().into(),
        input()["source_revision"].as_str().unwrap().into(),
    )
}
fn job(requests: &Arc<Requests>, clock: Arc<TestClock>, input: &Value) -> Job {
    requests
        .begin(
            &Uuid::new_v4().to_string(),
            &operation("cloudstream", input),
            clock,
        )
        .unwrap()
}
fn request(
    home: &GuestHome,
    input: &Value,
    response: Result<Value, ApiError>,
) -> Result<Value, ApiError> {
    let requests = Arc::new(Requests::default());
    let job = job(&requests, Arc::new(TestClock::default()), input);
    home.request(input, "host-epoch", "guest-epoch", &job, |_, _| response)
}
fn observe(kind: &str, input: &Value, page: &Value) {
    println!(
        "T06_DTO:{}",
        json!({"kind":kind, "synthetic":true, "input":input, "page":page})
    );
}

#[test]
fn t06_archived_identity_and_home_scope_vectors() {
    let f = fixture();
    let identity_vector = &f["identity_example"];
    let s = &identity_vector["section_input"];
    let parts = [
        identity_vector["source_id"].as_str().unwrap(),
        identity_vector["source_revision"].as_str().unwrap(),
        s["mainpage_name"].as_str().unwrap(),
        s["mainpage_data"].as_str().unwrap(),
        s["duplicate_section_ordinal"].as_str().unwrap(),
    ];
    assert_eq!(identity("section", &parts), identity_vector["section_id"]);
    let mut duplicate = parts;
    duplicate[4] = "1";
    assert_ne!(identity("section", &parts), identity("section", &duplicate));
    let mut different_data = parts;
    different_data[3] = "https://fixture.invalid/list/2";
    assert_ne!(
        identity("section", &parts),
        identity("section", &different_data)
    );
    for vector in f["http_identity_examples"].as_array().unwrap() {
        let parts: Vec<_> = vector["section_parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            identity(
                &format!("{}_section", vector["kind"].as_str().unwrap()),
                &parts
            ),
            vector["section_id"]
        );
    }
    let cases: Vec<_> = f["semantic_cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["kind"] == "home_response")
        .collect();
    assert_eq!(cases.len(), 4);
    for case in cases {
        assert_eq!(
            validate(&case["request"], &case["value"]).is_ok(),
            case["expected_valid"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
    println!(
        "T06_IDENTITY:{}",
        json!({"input":identity_vector, "actual":identity("section", &parts)})
    );
}

#[test]
fn t06_three_kind_common_dto_and_negative_relationships() {
    for kind in ["cloudstream", "maccms", "stremio"] {
        let mut input = input();
        let mut good = vector()["value"].clone();
        if kind != "cloudstream" {
            let f = fixture();
            let ids = f["http_identity_examples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["kind"] == kind)
                .unwrap();
            for field in ["source_id", "source_revision"] {
                input[field] = ids[field].clone();
                good[field] = ids[field].clone();
                for section in good["sections"].as_array_mut().unwrap() {
                    for card in section["items"].as_array_mut().unwrap() {
                        card["ref"][field] = ids[field].clone();
                    }
                }
            }
        }
        validate(&input, &good).unwrap();
        observe(kind, &input, &good);
        for mutation in [
            "sort",
            "gap",
            "duplicate-index",
            "duplicate-ref",
            "duplicate-id",
            "page",
            "next",
            "has-next",
            "source",
            "revision",
            "card-source",
            "null-retry",
        ] {
            let mut bad = good.clone();
            match mutation {
                "sort" => bad["sections"].as_array_mut().unwrap().reverse(),
                "gap" => bad["sections"][1]["index"] = json!(3),
                "duplicate-index" => bad["failed_sections"][0]["index"] = json!(0),
                "duplicate-ref" => {
                    bad["sections"][1]["section_ref"] = bad["sections"][0]["section_ref"].clone()
                }
                "duplicate-id" => bad["sections"][1]["id"] = bad["sections"][0]["id"].clone(),
                "page" => bad["sections"][0]["page"] = json!(2),
                "next" => bad["sections"][0]["next_page"] = json!(2),
                "has-next" => bad["sections"][0]["has_next"] = json!(true),
                "source" => bad["source_id"] = json!("other-source"),
                "revision" => bad["source_revision"] = json!("other-revision"),
                "card-source" => {
                    bad["sections"][0]["items"][0]["ref"]["source_id"] = json!("other-source")
                }
                "null-retry" => bad["failed_sections"][0]["section_ref"] = Value::Null,
                _ => unreachable!(),
            }
            assert!(validate(&input, &bad).is_err(), "{kind}/{mutation}");
        }
    }
}

#[test]
fn t06_guest_dynamic_multi_duplicate_titles_and_original_refs() {
    let home = context();
    let mut first = section(0, "{\"request\":\"原值/%2F\",\"column\":0}");
    first["has_next"] = json!(true);
    first["next_page"] = json!(2);
    let second = section(1, "{\"request\":\"原值/%2F\",\"column\":1}");
    // A single bridge response carries multiple columns, with opaque guest
    // references identifying the same original request and distinct results.
    let full = request(
        &home,
        &input(),
        Ok(dto(vec![first.clone(), second], vec![])),
    )
    .unwrap();
    assert_eq!(full["sections"].as_array().unwrap().len(), 2);
    assert_eq!(full["sections"][0]["title"], full["sections"][1]["title"]);
    assert_ne!(full["sections"][0]["id"], full["sections"][1]["id"]);
    assert_ne!(
        full["sections"][0]["section_ref"],
        full["sections"][1]["section_ref"]
    );
    observe("cloudstream", &input(), &full);
    let mut paged = input();
    paged["section_ref"] = full["sections"][0]["section_ref"].clone();
    paged["page"] = json!(2);
    let requests = Arc::new(Requests::default());
    let j = job(&requests, Arc::new(TestClock::default()), &paged);
    let next = home
        .request(
            &paged,
            "host-epoch",
            "guest-epoch",
            &j,
            |forwarded, child| {
                assert_eq!(forwarded["section_ref"], first["section_ref"]);
                assert_eq!(forwarded["page"], 2);
                assert_eq!(child.budget.total_ms, 30000);
                assert_eq!(child.budget.child_ms, 15000);
                first["page"] = json!(2);
                first["has_next"] = json!(false);
                first["next_page"] = Value::Null;
                Ok(dto(vec![first.clone()], vec![]))
            },
        )
        .unwrap();
    assert_eq!(next["sections"][0]["section_ref"], paged["section_ref"]);
    assert_eq!(next["sections"][0]["index"], 0);
    observe("cloudstream", &paged, &next);
    paged["page"] = json!(3);
    assert_eq!(
        request(&home, &paged, Ok(dto(vec![], vec![])))
            .unwrap_err()
            .code,
        "BAD_REQUEST"
    );
    let refreshed = request(
        &home,
        &input(),
        Ok(dto(vec![section(0, "dynamic-new")], vec![])),
    )
    .unwrap();
    assert_eq!(refreshed["sections"].as_array().unwrap().len(), 1);
    paged["page"] = json!(1);
    assert_eq!(
        request(&home, &paged, Ok(dto(vec![first], vec![])))
            .unwrap_err()
            .code,
        "BAD_REQUEST"
    );
}

#[test]
fn t06_guest_partial_retry_scope_and_empty_all_failure() {
    let home = context();
    let full = request(&home, &input(), Ok(vector()["value"].clone())).unwrap();
    assert_eq!(full["sections"][0]["index"], 0);
    assert_eq!(full["sections"][1]["index"], 2);
    assert_eq!(full["failed_sections"][0]["index"], 1);
    let mut retry = input();
    retry["section_ref"] = full["failed_sections"][0]["section_ref"].clone();
    assert_eq!(
        request(
            &home,
            &retry,
            Err(ApiError::new("UPSTREAM_FAILED", "synthetic failure"))
        )
        .unwrap_err()
        .code,
        "UPSTREAM_FAILED"
    );
    let repaired = request(&home, &retry, Ok(dto(vec![section(1, "ref-b")], vec![]))).unwrap();
    assert_eq!(repaired["sections"].as_array().unwrap().len(), 1);
    assert_eq!(repaired["sections"][0]["index"], 1);
    assert_eq!(repaired["failed_sections"], json!([]));
    observe("cloudstream", &retry, &repaired);
    // The scoped result supplies only B. Original A/C values stay byte-exact
    // for a consumer applying this response at index 1.
    assert_eq!(full["sections"][0]["title"], "A");
    assert_eq!(full["sections"][1]["title"], "C");
    let all_failed = dto(
        vec![],
        vec![failure(
            "A",
            "a",
            0,
            &ApiError::new("UPSTREAM_FAILED", "synthetic"),
        )],
    );
    assert_eq!(
        request(&home, &input(), Ok(all_failed)).unwrap_err().code,
        "UPSTREAM_FAILED"
    );
    let empty = request(&home, &input(), Ok(dto(vec![], vec![]))).unwrap();
    assert_eq!(empty["sections"], json!([]));
    let empty_column =
        request(&home, &input(), Ok(dto(vec![section(0, "empty")], vec![]))).unwrap();
    assert_eq!(empty_column["sections"][0]["items"], json!([]));
    assert_eq!(empty_column["sections"][0]["has_next"], false);
}

#[test]
fn t06_guest_epoch_revision_and_transactional_negatives() {
    let home = context();
    let full = request(&home, &input(), Ok(dto(vec![section(0, "stable")], vec![]))).unwrap();
    let mut retry = input();
    retry["section_ref"] = full["sections"][0]["section_ref"].clone();
    for change in ["index", "id", "ref", "multiple", "failures"] {
        let mut response = dto(vec![section(0, "stable")], vec![]);
        match change {
            "index" => response["sections"][0]["index"] = json!(1),
            "id" => response["sections"][0]["id"] = json!("wrong"),
            "ref" => response["sections"][0]["section_ref"] = json!("wrong"),
            "multiple" => response["sections"]
                .as_array_mut()
                .unwrap()
                .push(section(1, "other")),
            "failures" => {
                response["failed_sections"] = json!([failure(
                    "failed",
                    "other",
                    1,
                    &ApiError::new("UPSTREAM_FAILED", "synthetic")
                )])
            }
            _ => unreachable!(),
        }
        assert_eq!(
            request(&home, &retry, Ok(response)).unwrap_err().code,
            "UPSTREAM_FAILED",
            "{change}"
        );
        request(&home, &retry, Ok(dto(vec![section(0, "stable")], vec![]))).unwrap();
    }
    let requests = Arc::new(Requests::default());
    let j = job(&requests, Arc::new(TestClock::default()), &retry);
    for (host_epoch, guest_epoch) in [("old-host", "guest-epoch"), ("host-epoch", "old-guest")] {
        assert_eq!(
            home.request(&retry, host_epoch, guest_epoch, &j, |_, _| panic!(
                "stale call reached transport"
            ))
            .unwrap_err()
            .code,
            "CANCELLED"
        );
    }
    let mut wrong = retry.clone();
    wrong["source_revision"] = json!("old-revision");
    assert_eq!(
        request(&home, &wrong, Ok(dto(vec![], vec![])))
            .unwrap_err()
            .code,
        "REVISION_MISMATCH"
    );
    let other = GuestHome::new(
        "new-host".into(),
        "guest-epoch".into(),
        input()["source_id"].as_str().unwrap().into(),
        input()["source_revision"].as_str().unwrap().into(),
    );
    assert_eq!(
        other
            .request(&retry, "new-host", "guest-epoch", &j, |_, _| panic!(
                "foreign handle reached transport"
            ))
            .unwrap_err()
            .code,
        "BAD_REQUEST"
    );
}

#[test]
fn t06_provider_serial_queue_deadline_and_cancelled_publication() {
    let home = Arc::new(context());
    let requests = Arc::new(Requests::default());
    let clock = Arc::new(TestClock::default());
    let first_job = job(&requests, clock.clone(), &input());
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let (entered, waiting) = mpsc::channel();
    let (release, proceed) = mpsc::channel();
    let worker_home = home.clone();
    let worker_active = active.clone();
    let worker_max = maximum.clone();
    let first = std::thread::spawn(move || {
        worker_home.request(&input(), "host-epoch", "guest-epoch", &first_job, |_, _| {
            let n = worker_active.fetch_add(1, Ordering::SeqCst) + 1;
            worker_max.fetch_max(n, Ordering::SeqCst);
            entered.send(()).unwrap();
            proceed.recv_timeout(Duration::from_secs(5)).unwrap();
            worker_active.fetch_sub(1, Ordering::SeqCst);
            Ok(dto(vec![section(0, "first")], vec![]))
        })
    });
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    let second_job = job(&requests, clock.clone(), &input());
    let second_id = Uuid::new_v4().to_string();
    // A separately admitted full request queues behind the first provider call.
    let worker_home = home.clone();
    let second_max = maximum.clone();
    let (second_entered, second_waiting) = mpsc::channel();
    let second = std::thread::spawn(move || {
        worker_home.request(
            &input(),
            "host-epoch",
            "guest-epoch",
            &second_job,
            |_, _| {
                let n = active.fetch_add(1, Ordering::SeqCst) + 1;
                second_max.fetch_max(n, Ordering::SeqCst);
                second_entered.send(()).unwrap();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(dto(vec![section(0, "second")], vec![]))
            },
        )
    });
    assert!(
        second_waiting
            .recv_timeout(Duration::from_millis(30))
            .is_err()
    );
    release.send(()).unwrap();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    second_waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
    assert_eq!(home.bindings.lock().unwrap()[0].guest, "second");
    assert_eq!(requests.active_count(), 0);
    // Cancellation after a completed transport must prevent registry publication.
    let cancelled = requests
        .begin(&second_id, "cloudstream_home", clock.clone())
        .unwrap();
    assert_eq!(
        home.request(&input(), "host-epoch", "guest-epoch", &cancelled, |_, _| {
            requests
                .cancel(&Uuid::new_v4().to_string(), &second_id)
                .unwrap();
            Ok(dto(vec![section(0, "cancelled")], vec![]))
        })
        .unwrap_err()
        .code,
        "CANCELLED"
    );
    assert_eq!(home.bindings.lock().unwrap()[0].guest, "second");
}

#[test]
fn t06_provider_queue_timeout_and_cancel_do_not_enter_transport() {
    for mode in ["deadline", "cancel"] {
        let home = Arc::new(context());
        let full = request(
            &home,
            &input(),
            Ok(dto(vec![section(0, "registered")], vec![])),
        )
        .unwrap();
        let mut retry = input();
        retry["section_ref"] = full["sections"][0]["section_ref"].clone();
        let requests = Arc::new(Requests::default());
        let clock = Arc::new(TestClock::default());
        let first_job = job(&requests, clock.clone(), &input());
        let worker_home = home.clone();
        let (entered, waiting) = mpsc::channel();
        let (release, proceed) = mpsc::channel();
        let first = std::thread::spawn(move || {
            worker_home.request(&input(), "host-epoch", "guest-epoch", &first_job, |_, _| {
                entered.send(()).unwrap();
                proceed.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(dto(vec![section(0, "registered")], vec![]))
            })
        });
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        let pending_id = Uuid::new_v4().to_string();
        let pending = requests
            .begin(&pending_id, "cloudstream_home_page", clock.clone())
            .unwrap();
        let worker_home = home.clone();
        let (finished, result) = mpsc::channel();
        let second = std::thread::spawn(move || {
            finished
                .send(
                    worker_home.request(&retry, "host-epoch", "guest-epoch", &pending, |_, _| {
                        panic!("expired/cancelled queued request reached transport")
                    }),
                )
                .unwrap();
        });
        if mode == "deadline" {
            clock.0.store(30000, Ordering::SeqCst);
        } else {
            requests
                .cancel(&Uuid::new_v4().to_string(), &pending_id)
                .unwrap();
        }
        let response = result.recv_timeout(Duration::from_secs(2));
        release.send(()).unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap();
        assert_eq!(
            response.unwrap().unwrap_err().code,
            if mode == "deadline" {
                "UPSTREAM_TIMEOUT"
            } else {
                "CANCELLED"
            }
        );
        assert_eq!(requests.active_count(), 0);
    }
}

#[test]
fn t06_budget_selection_child_deadline_reserve_and_cleanup() {
    let requests = Arc::new(Requests::default());
    for (kind, total, child, slots) in [
        ("cloudstream", 60000, 15000, 1),
        ("maccms", 45000, 30000, 4),
        ("stremio", 45000, 30000, 4),
    ] {
        let clock = Arc::new(TestClock::default());
        let name = operation(kind, &input());
        assert_eq!(
            (
                budgets()[&name].total_ms,
                budgets()[&name].child_ms,
                budgets()[&name].max_concurrency
            ),
            (total, child, slots)
        );
        let parent = requests
            .begin(&Uuid::new_v4().to_string(), &name, clock.clone())
            .unwrap();
        let result = column(&parent, true, |job| {
            assert_eq!(job.remaining_ms().unwrap(), child);
            clock.0.store(child - 1, Ordering::SeqCst);
            assert_eq!(job.remaining_ms().unwrap(), 1);
            clock.0.store(child, Ordering::SeqCst);
            job.check()
        });
        assert_eq!(result.unwrap_err().code, "UPSTREAM_TIMEOUT");
        drop(parent);
        assert_eq!(requests.active_count(), 0);
        let clock = Arc::new(TestClock::default());
        let parent = requests
            .begin(&Uuid::new_v4().to_string(), &name, clock.clone())
            .unwrap();
        clock.0.store(total - 1000, Ordering::SeqCst);
        assert_eq!(
            reserve_error(&parent).unwrap().unwrap().code,
            "UPSTREAM_TIMEOUT"
        );
        assert_eq!(parent.remaining_ms().unwrap(), 1000);
        assert_eq!(
            column::<()>(&parent, true, |_| panic!("work started in reserve"))
                .unwrap_err()
                .code,
            "UPSTREAM_TIMEOUT"
        );
        drop(parent);
        let mut single = input();
        single["section_ref"] = json!("ref");
        for page in [1, 2] {
            single["page"] = json!(page);
            let name = operation(kind, &single);
            assert_eq!(budgets()[&name].total_ms, 30000);
            assert_eq!(
                budgets()[&name].child_ms,
                if kind == "cloudstream" { 15000 } else { 30000 }
            );
        }
    }
    assert_eq!(requests.active_count(), 0);
}
