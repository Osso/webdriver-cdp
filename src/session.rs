use crate::cdp::CdpSession;
use serde_json::Value;

pub struct Session {
    pub session_id: String,
    pub target_id: String,
    pub cdp: CdpSession,
    pub implicit_wait_ms: u64,
    pub page_load_timeout_ms: u64,
    pub script_timeout_ms: u64,
}

impl Session {
    pub fn new(session_id: String, target_id: String, cdp: CdpSession) -> Self {
        Self {
            session_id,
            target_id,
            cdp,
            implicit_wait_ms: 0,
            page_load_timeout_ms: 300_000,
            script_timeout_ms: 30_000,
        }
    }

    /// Evaluate JS and return the result value.
    pub async fn evaluate_js(
        &self,
        expression: &str,
        return_by_value: bool,
    ) -> Result<Value, crate::error::WebDriverError> {
        self.cdp
            .send_command(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": return_by_value,
                    "awaitPromise": false,
                }),
            )
            .await
    }

    /// Call a function on a remote object.
    pub async fn call_function_on(
        &self,
        object_id: &str,
        function_declaration: &str,
        args: Vec<Value>,
        return_by_value: bool,
    ) -> Result<Value, crate::error::WebDriverError> {
        let cdp_args: Vec<Value> = args
            .into_iter()
            .map(|a| serde_json::json!({"value": a}))
            .collect();
        self.cdp
            .send_command(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "objectId": object_id,
                    "functionDeclaration": function_declaration,
                    "arguments": cdp_args,
                    "returnByValue": return_by_value,
                    "awaitPromise": false,
                }),
            )
            .await
    }
}
