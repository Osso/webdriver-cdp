use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, Clone)]
pub enum WebDriverError {
    SessionNotCreated(String),
    InvalidSessionId,
    NoSuchElement,
    NoSuchFrame,
    StaleElementReference,
    ElementNotInteractable,
    InvalidArgument(String),
    JavascriptError(String),
    Timeout,
    NoSuchWindow,
    NoSuchCookie(String),
    UnknownCommand(String),
    UnknownError(String),
    NoSuchAlert,
    ElementClickIntercepted,
    InsecureCertificate,
    MoveTargetOutOfBounds,
}

impl WebDriverError {
    pub fn error_code(&self) -> &str {
        match self {
            Self::SessionNotCreated(_) => "session not created",
            Self::InvalidSessionId => "invalid session id",
            Self::NoSuchElement => "no such element",
            Self::NoSuchFrame => "no such frame",
            Self::StaleElementReference => "stale element reference",
            Self::ElementNotInteractable => "element not interactable",
            Self::InvalidArgument(_) => "invalid argument",
            Self::JavascriptError(_) => "javascript error",
            Self::Timeout => "timeout",
            Self::NoSuchWindow => "no such window",
            Self::NoSuchCookie(_) => "no such cookie",
            Self::UnknownCommand(_) => "unknown command",
            Self::UnknownError(_) => "unknown error",
            Self::NoSuchAlert => "no such alert",
            Self::ElementClickIntercepted => "element click intercepted",
            Self::InsecureCertificate => "insecure certificate",
            Self::MoveTargetOutOfBounds => "move target out of bounds",
        }
    }

    pub fn http_status(&self) -> StatusCode {
        match self {
            Self::SessionNotCreated(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidSessionId => StatusCode::NOT_FOUND,
            Self::NoSuchElement => StatusCode::NOT_FOUND,
            Self::NoSuchFrame => StatusCode::NOT_FOUND,
            Self::StaleElementReference => StatusCode::NOT_FOUND,
            Self::ElementNotInteractable => StatusCode::BAD_REQUEST,
            Self::InvalidArgument(_) => StatusCode::BAD_REQUEST,
            Self::JavascriptError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::NoSuchWindow => StatusCode::NOT_FOUND,
            Self::NoSuchCookie(_) => StatusCode::NOT_FOUND,
            Self::UnknownCommand(_) => StatusCode::NOT_FOUND,
            Self::UnknownError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoSuchAlert => StatusCode::NOT_FOUND,
            Self::ElementClickIntercepted => StatusCode::BAD_REQUEST,
            Self::InsecureCertificate => StatusCode::BAD_REQUEST,
            Self::MoveTargetOutOfBounds => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::SessionNotCreated(m) => m.clone(),
            Self::InvalidSessionId => "Session not found".into(),
            Self::NoSuchElement => "Element not found".into(),
            Self::NoSuchFrame => "Frame not found".into(),
            Self::StaleElementReference => "Element is stale".into(),
            Self::ElementNotInteractable => "Element not interactable".into(),
            Self::InvalidArgument(m) => m.clone(),
            Self::JavascriptError(m) => m.clone(),
            Self::Timeout => "Operation timed out".into(),
            Self::NoSuchWindow => "Window not found".into(),
            Self::NoSuchCookie(m) => m.clone(),
            Self::UnknownCommand(m) => m.clone(),
            Self::UnknownError(m) => m.clone(),
            Self::NoSuchAlert => "No alert present".into(),
            Self::ElementClickIntercepted => "Click intercepted".into(),
            Self::InsecureCertificate => "Insecure certificate".into(),
            Self::MoveTargetOutOfBounds => "Move target out of bounds".into(),
        }
    }
}

impl IntoResponse for WebDriverError {
    fn into_response(self) -> Response {
        let body = json!({
            "value": {
                "error": self.error_code(),
                "message": self.message(),
                "stacktrace": ""
            }
        });
        (self.http_status(), axum::Json(body)).into_response()
    }
}

impl std::fmt::Display for WebDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message())
    }
}

impl std::error::Error for WebDriverError {}
