//! Shared by the MacCMS/Stremio fixture observers and their nested card tests.
use serde_json::{Value, json};

const PLACEHOLDER: &str = "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr";

fn credential(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().replace('-', "_").as_str(),
        "cap"
            | "capability"
            | "media_capability"
            | "token"
            | "control_token"
            | "access_token"
            | "refresh_token"
            | "bearer"
            | "authorization"
            | "cookie"
            | "cookies"
            | "set_cookie"
    )
}

pub(super) fn redact(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if credential(key) {
                    *value = json!(PLACEHOLDER);
                } else {
                    redact(value);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                redact(value);
            }
        }
        Value::String(text) => {
            if let Ok(mut url) = url::Url::parse(text)
                && url.query_pairs().any(|(key, _)| credential(&key))
            {
                let pairs: Vec<_> = url
                    .query_pairs()
                    .map(|(key, value)| {
                        let value = if credential(&key) {
                            PLACEHOLDER.into()
                        } else {
                            value.into_owned()
                        };
                        (key.into_owned(), value)
                    })
                    .collect();
                url.query_pairs_mut().clear().extend_pairs(pairs);
                *text = url.into();
            }
        }
        _ => {}
    }
}

pub(super) fn diagnostic(value: &Value) -> Value {
    let mut safe = value.clone();
    redact(&mut safe);
    safe
}

#[test]
fn credentials_in_nested_observations_and_failure_debug_are_redacted() {
    let secret = "s".repeat(43);
    let original = json!({
        "stable": {"session_id":"public-session-id", "state":"ready", "count":200},
        "response": [{"media_capability":secret, "Authorization":"Bearer private-control",
            "cookie":"provider=private", "Set-Cookie":"provider=private",
            "session":{"media_url":format!("http://127.0.0.1:1234/media/session/stream?cap={secret}"),
                "control_token":"private-token"},
            "images":[{"url":format!("http://127.0.0.1:1234/images/image?keep=a%2Fb&%63ap={secret}")}],
            "nested":{"capability":secret,"token":"private-token"}}]
    });
    let snapshot = original.clone();
    let safe = diagnostic(&original);
    assert_eq!(original, snapshot);
    assert_eq!(safe["stable"], original["stable"]);
    let output = format!("{safe} {safe:?}");
    for secret in [
        secret.as_str(),
        "private-control",
        "provider=private",
        "private-token",
    ] {
        assert!(!output.contains(secret));
    }
    assert_eq!(safe["response"][0]["media_capability"], PLACEHOLDER);
    assert_eq!(safe["response"][0]["session"]["control_token"], PLACEHOLDER);
    let url = url::Url::parse(safe["response"][0]["images"][0]["url"].as_str().unwrap()).unwrap();
    assert_eq!(
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>(),
        [
            ("keep".into(), "a/b".into()),
            ("cap".into(), PLACEHOLDER.into())
        ]
    );
    assert_eq!(diagnostic(&safe), safe);
}
