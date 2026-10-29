//! The working schema is also the runtime validator. No network schema retrieval.
use jsonschema::{Resource, Validator};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::OnceLock};

pub const APP: &str = include_str!("../../protocol/app.schema.json");
pub const BRIDGE: &str = include_str!("../../protocol/bridge.openapi.json");
pub const COMMANDS: &[&str] = &[
    "runtime_status",
    "runtime_prepare",
    "get_sources",
    "sync_catalog",
    "get_home",
    "get_details",
    "resolve_episode",
    "open_playback",
    "close_playback",
    "cancel_request",
    "start_interaction",
    "finish_interaction",
    "get_settings",
    "save_settings",
    "get_resource_status",
    "release_resolution",
    "match_playback",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub details: Option<Value>,
}

impl ApiError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn bad(message: &str) -> Self {
        Self::new("BAD_REQUEST", message)
    }
    pub fn internal(message: &str) -> Self {
        Self::new("INTERNAL_ERROR", message)
    }
    pub fn cancelled() -> Self {
        Self::new(
            "CANCELLED",
            "Request or captured source generation is obsolete",
        )
    }
}

pub struct Contracts {
    validators: HashMap<String, Validator>,
}

impl Contracts {
    pub fn get() -> &'static Self {
        static CONTRACTS: OnceLock<Contracts> = OnceLock::new();
        CONTRACTS.get_or_init(|| Self::build().expect("Invalid embedded working protocol"))
    }

    fn build() -> Result<Self, Box<dyn std::error::Error>> {
        let app: Value = serde_json::from_str(APP)?;
        let mut bridge: Value = serde_json::from_str(BRIDGE)?;
        bridge["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        bridge["$id"] = json!("https://replica.invalid/contracts/bridge.openapi.json");
        let mut validators = HashMap::new();
        for name in app["$defs"]
            .as_object()
            .ok_or("Missing definitions")?
            .keys()
        {
            let validator = jsonschema::draft202012::options()
                .should_validate_formats(true)
                .with_resource(app["$id"].as_str().ok_or("Missing app id")?, Resource::from_contents(app.clone())?)
                .with_resource("https://replica.invalid/contracts/bridge.openapi.json", Resource::from_contents(bridge.clone())?)
                .build(&json!({"$ref": format!("https://replica.invalid/contracts/app.schema.json#/$defs/{name}")}))?;
            validators.insert(name.clone(), validator);
        }
        Ok(Self { validators })
    }

    pub fn validate(&self, definition: &str, value: &Value) -> Result<(), ApiError> {
        let validator = self
            .validators
            .get(definition)
            .ok_or_else(|| ApiError::bad("Unknown command or DTO"))?;
        if !validator.is_valid(value) {
            // Validator error text can contain signed URLs and opaque data.
            return Err(ApiError::bad(&format!(
                "Value does not satisfy {definition}"
            )));
        }
        Ok(())
    }
}

pub fn envelope(request_id: &Value, host_epoch: &str, result: Result<Value, ApiError>) -> Value {
    match result {
        Ok(data) => json!({"request_id": request_id, "engine_epoch": host_epoch, "data": data}),
        Err(error) => json!({"request_id": request_id, "engine_epoch": host_epoch, "error": error}),
    }
}

pub fn authorized(label: &str, url: &url::Url) -> bool {
    label == "main"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && ((url.scheme() == "tauri" && url.host_str() == Some("localhost"))
            || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost")))
}

pub fn wire_size(value: &Value, limit: usize) -> Result<(), ApiError> {
    if serde_json::to_vec(value)
        .map_err(|_| ApiError::bad("Invalid JSON"))?
        .len()
        > limit
    {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Wire JSON byte budget exceeded",
        ));
    }
    fn opaque(value: &Value) -> bool {
        match value {
            Value::Object(fields) => fields.iter().all(|(key, value)| {
                !(key == "data" && value.as_str().is_some_and(|text| text.len() > 524_288))
                    && opaque(value)
            }),
            Value::Array(items) => items.iter().all(opaque),
            _ => true,
        }
    }
    if !opaque(value) {
        return Err(ApiError::new(
            "RESOURCE_LIMIT",
            "Opaque data UTF-8 byte budget exceeded",
        ));
    }
    Ok(())
}
