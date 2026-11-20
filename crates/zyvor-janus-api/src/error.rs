// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use zyvor_janus_config::ConfigError;

/// API error mapped to an HTTP response shaped `{"detail": "..."}`, matching
/// the existing FastAPI server's `HTTPException` JSON shape so `web/`'s
/// error parsing needs no changes.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub detail: String,
}

impl ApiError {
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            detail: detail.into(),
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail: detail.into(),
        }
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            detail: detail.into(),
        }
    }

    pub fn too_many_requests(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            detail: detail.into(),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: detail.into(),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    detail: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            axum::Json(ErrorBody {
                detail: self.detail,
            }),
        )
            .into_response()
    }
}

impl From<ConfigError> for ApiError {
    fn from(err: ConfigError) -> Self {
        ApiError::internal(err.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::internal(err.to_string())
    }
}
