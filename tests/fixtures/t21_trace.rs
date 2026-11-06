//! Optional T21 observer on the existing real Host/WKWebView fixture.
//! Only opaque test context IDs and owned filenames are persisted.
use super::*;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

pub(super) struct Trace {
    media: PathBuf,
    owners: Mutex<HashMap<String, Value>>,
    bindings: Mutex<Vec<Value>>,
    requests: Mutex<Vec<Value>>,
}

impl Trace {
    pub(super) fn from_environment() -> Option<Arc<Self>> {
        std::env::var_os("T21_MEDIA").map(|path| {
            let media = PathBuf::from(path);
            assert!(
                media.is_dir(),
                "T21 requires the shared owned media directory"
            );
            Arc::new(Self {
                media,
                owners: Mutex::new(HashMap::new()),
                bindings: Mutex::new(Vec::new()),
                requests: Mutex::new(Vec::new()),
            })
        })
    }

    pub(super) fn bind(&self, view: &Value, link: &Link) {
        let context = link.headers["x-t21-context"].to_str().unwrap().to_owned();
        let url = url::Url::parse(&link.url).unwrap();
        let episode = url
            .query_pairs()
            .find(|(key, _)| key == "episode")
            .unwrap()
            .1
            .parse::<usize>()
            .unwrap();
        let owner = json!({
            "context":context, "session_id":view["session_id"],
            "resolution_id":view["resolution_id"], "stream_id":view["stream_id"],
            "episode":episode, "root":url.path().trim_start_matches("/media/")
        });
        self.owners.lock().unwrap().insert(context, owner.clone());
        self.bindings.lock().unwrap().push(owner);
    }

    pub(super) fn request(&self, request: &Request, reply: &Reply, mode: usize) {
        assert!(request.target.starts_with("/media/"));
        let context = request
            .headers
            .get("x-t21-context")
            .expect("media subrequest lost its native session context");
        let owner = self
            .owners
            .lock()
            .unwrap()
            .get(context)
            .expect("media subrequest has no issued session")
            .clone();
        let name = request
            .target
            .trim_start_matches("/media/")
            .split('?')
            .next()
            .unwrap();
        assert!(
            !name.contains('/') && !name.contains(".."),
            "unregistered media path"
        );
        if name.ends_with(".ts") {
            assert_eq!(
                owner["root"], "index.m3u8",
                "segment belongs to a progressive session"
            );
            let playlist = std::fs::read_to_string(self.media.join("index.m3u8")).unwrap();
            assert!(
                playlist.lines().any(|line| line == name),
                "wrong HLS subrequest"
            );
        } else {
            assert_eq!(
                owner["root"], name,
                "root request belongs to another stream"
            );
            assert_eq!(
                request
                    .query
                    .get("episode")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap(),
                owner["episode"].as_u64().unwrap() as usize,
                "root request belongs to another episode"
            );
        }
        assert!(
            !request.headers.contains_key("authorization"),
            "local authorization reached upstream"
        );
        assert!(
            !request.headers.contains_key("cookie"),
            "browser Cookie reached upstream"
        );
        assert!(
            !request.query.contains_key("cap"),
            "local capability reached upstream query"
        );
        self.requests.lock().unwrap().push(json!({
            "owner":owner, "name":name, "mode":mode, "status":reply.status,
            "bytes":reply.body.len(), "sha256":format!("{:x}", Sha256::digest(&reply.body)),
            "range":request.headers.get("range"), "local_credentials":false
        }));
    }

    pub(super) fn finish(&self, active_requests: usize, upstream_active: usize) {
        let requests = self.requests.lock().unwrap();
        assert!(!requests.is_empty());
        let inventory: Vec<_> = std::fs::read_dir(&self.media)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = std::fs::read(&path).unwrap();
                json!({"name":path.file_name().unwrap().to_str().unwrap(), "bytes":bytes.len(),
                "sha256":format!("{:x}", Sha256::digest(bytes))})
            })
            .collect();
        println!(
            "T21_VECTOR:{}",
            json!({"inventory":inventory,
            "bindings":*self.bindings.lock().unwrap(), "requests":*requests,
            "active_requests":active_requests, "upstream_active":upstream_active,
            "local_credentials":false})
        );
    }
}
