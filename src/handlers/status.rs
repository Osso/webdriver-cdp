use axum::Json;
use serde_json::{Value, json};

pub async fn get_status() -> Json<Value> {
    Json(json!({
        "value": {
            "ready": true,
            "message": "webdriver-cdp"
        }
    }))
}
