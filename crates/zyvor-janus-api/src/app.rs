// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use axum::routing::get;
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::{auth, routes};

/// Builds the full router (extracted from `main` so handler tests can drive it).
pub fn build_app(state: AppState) -> Router {
    // `/api/health` is intentionally NOT behind the bearer-auth layer --
    // K8s liveness/readiness probes (deploy/kubernetes/api.yaml) call it
    // with no credentials, matching the Python server's behavior.
    let public_routes = Router::new().route("/api/health", get(routes::health::health));

    let authenticated_routes = Router::new()
        .route("/api/configs", get(routes::configs::list_configs))
        .route(
            "/api/runs",
            get(routes::runs::list_runs).post(routes::runs::start_run),
        )
        .route("/api/runs/{id}", get(routes::runs::get_run))
        .route("/api/runs/{id}/snapshots", get(routes::runs::get_snapshots))
        .route("/api/runs/{id}/timeline", get(routes::runs::get_timeline))
        .route("/api/runs/{id}/events", get(routes::runs::get_events))
        .route(
            "/api/runs/{id}/shadow-events",
            get(routes::runs::get_shadow_events),
        )
        .route("/api/compare", axum::routing::post(routes::batch::compare))
        .route(
            "/api/workloads/generate",
            axum::routing::post(routes::analysis::generate),
        )
        .route(
            "/api/load-sweep",
            axum::routing::post(routes::analysis::load_sweep_route),
        )
        .route(
            "/api/benchmark/presets",
            get(routes::batch::benchmark_presets),
        )
        .route(
            "/api/benchmark/reports",
            get(routes::batch::benchmark_reports),
        )
        .route(
            "/api/benchmark/run",
            axum::routing::post(routes::batch::benchmark_run),
        )
        .route("/api/what-if", axum::routing::post(routes::batch::what_if))
        .route(
            "/api/runs/{id}/serving-trace",
            get(routes::serving_trace::get_serving_trace),
        )
        .route("/api/twins", get(routes::twins::get_twins))
        .route(
            "/api/runs/{id}/ws-ticket",
            axum::routing::post(routes::runs::create_ws_ticket),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer_token,
        ));

    // Deliberately outside the bearer-auth layer -- see routes::ws for why
    // (browsers can't set Authorization on a WS handshake). Ticket-verified
    // per-connection instead.
    let ws_routes = Router::new().route("/ws/runs/{id}", get(routes::ws::ws_run));

    // The OpenAI shim gets its own bearer-check middleware (see
    // routes::openai_shim::require_shim_api_key), kept independent of the
    // blanket `/api/*` layer above -- not merged into `authenticated_routes`.
    let shim_routes = Router::new()
        .route(
            "/v1/chat/completions",
            axum::routing::post(routes::openai_shim::chat_completions),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::openai_shim::require_shim_api_key,
        ));

    public_routes
        .merge(authenticated_routes)
        .merge(ws_routes)
        .merge(shim_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Method, Request, StatusCode};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use tower::ServiceExt;

    const KEY: &str = "test-key";

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_state() -> AppState {
        let root = repo_root();
        AppState::new(
            root.clone(),
            root.join("configs/clusters"),
            std::env::temp_dir().join("janus-api-test-runs"),
            root.join("configs/profiles"),
            KEY.to_string(),
            "test-ws-secret".to_string(),
            120,
        )
    }

    async fn call(
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let resp = build_app(test_state()).oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    async fn post(uri: &str, body: Value) -> (StatusCode, Value) {
        call(Method::POST, uri, Some(KEY), Some(body)).await
    }

    #[tokio::test]
    async fn health_is_public_but_api_needs_the_bearer_token() {
        let (s, v) = call(Method::GET, "/api/health", None, None).await;
        assert_eq!((s, v["status"].as_str()), (StatusCode::OK, Some("ok")));
        let (s, _) = call(Method::GET, "/api/configs", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _) = call(Method::GET, "/api/configs", Some("wrong"), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, v) = call(Method::GET, "/api/configs", Some(KEY), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(v
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "backfill_demo.yaml"));
    }

    #[tokio::test]
    async fn benchmark_run_with_easy_returns_quality_chargeback_and_slo() {
        let (s, v) = post(
            "/api/benchmark/run",
            json!({"config": "backfill_demo.yaml", "scheduler": "easy"}),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["metrics"]["jobs_completed"], 4);
        let q = &v["quality"];
        assert_eq!(q["jobs_considered"], 4);
        assert!(q["jct_mean"].as_f64().unwrap() > 0.0);
        assert!(q["jct_p99"].as_f64().unwrap() >= q["jct_p50"].as_f64().unwrap());
        assert!(q["ftf_rho_max"].as_f64().unwrap() > 0.0);
        let c = &v["chargeback"];
        assert!(c["used_gpu_hours"].as_f64().unwrap() > 0.0);
        assert!(c["tenants"].as_array().unwrap().len() >= 2);
        let slo = &v["slo"];
        // backfill_demo.yaml sets slo.max_wait_secs = 60: under easy only
        // `wide` and `short-small` start within 60 s.
        assert_eq!(slo["jobs_total"], 4);
        assert_eq!(slo["jobs_met"], 2);
        assert_eq!(slo["attainment"], 0.5);
        assert_eq!(slo["spec"]["max_wait_secs"], 60.0);
    }

    #[tokio::test]
    async fn easy_protects_the_head_job_that_fifo_delays_via_what_if() {
        let (s, v) = post(
            "/api/what-if",
            json!({"base_config": "backfill_demo.yaml", "schedulers": ["fifo", "easy", "sjf", "fairshare"]}),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let rows = v["results"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        let wait = |name: &str| {
            rows.iter().find(|r| r["scheduler"] == name).unwrap()["metrics"]["mean_wait_time"]
                .as_f64()
                .unwrap()
        };
        assert!(rows.iter().all(|r| r["status"] == "completed"));
        // fifo lets the 500 s job in ahead of `head`; easy does not.
        assert!(
            wait("easy") < wait("fifo"),
            "{} vs {}",
            wait("easy"),
            wait("fifo")
        );
        assert_eq!(wait("sjf"), wait("fifo"));
    }

    #[tokio::test]
    async fn run_detail_exposes_new_report_fields() {
        let app = build_app(test_state());
        let req = |method: Method, uri: String, body: Option<Value>| {
            let b = Request::builder()
                .method(method)
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {KEY}"))
                .header(header::CONTENT_TYPE, "application/json");
            b.body(
                body.map(|v| Body::from(v.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap()
        };
        let resp = app
            .clone()
            .oneshot(req(
                Method::POST,
                "/api/runs".into(),
                Some(json!({"config": "backfill_demo.yaml", "scheduler": "sjf"})),
            ))
            .await
            .unwrap();
        let id =
            serde_json::from_slice::<Value>(&to_bytes(resp.into_body(), 1 << 20).await.unwrap())
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string();
        let mut detail = Value::Null;
        for _ in 0..200 {
            let resp = app
                .clone()
                .oneshot(req(Method::GET, format!("/api/runs/{id}"), None))
                .await
                .unwrap();
            detail = serde_json::from_slice(&to_bytes(resp.into_body(), 64 << 20).await.unwrap())
                .unwrap();
            if detail["status"] == "completed" || detail["status"] == "failed" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(detail["status"], "completed", "{detail}");
        assert_eq!(detail["scheduler"], "sjf");
        assert!(detail["quality"]["jct_mean"].as_f64().unwrap() > 0.0);
        assert!(detail["chargeback"]["tenants"].is_array());
        assert!(detail["slo"]["attainment"].is_number());
    }

    #[tokio::test]
    async fn unknown_and_traversing_config_names_are_rejected() {
        let (s, _) = post(
            "/api/benchmark/run",
            json!({"config": "nope.yaml", "scheduler": "fifo"}),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        for bad in [
            "../clusters/backfill_demo.yaml",
            "a/b.yaml",
            "..",
            "a\\b.yaml",
            "",
        ] {
            let (s, v) = post("/api/runs", json!({"config": bad})).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {v}");
        }
    }

    #[tokio::test]
    async fn unsupported_scheduler_fails_the_run_not_the_server() {
        let (s, v) = post(
            "/api/benchmark/run",
            json!({"config": "backfill_demo.yaml", "scheduler": "bogus"}),
        )
        .await;
        assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(v["detail"].as_str().unwrap().contains("bogus"));
    }

    #[tokio::test]
    async fn generate_workload_is_deterministic_and_validates() {
        let spec = json!({"jobs": 25, "seed": 3, "arrival_rate_per_sec": 0.2,
                          "median_runtime_secs": 120.0, "tenants": 3});
        let (s, a) = post("/api/workloads/generate", spec.clone()).await;
        assert_eq!(s, StatusCode::OK, "{a}");
        assert_eq!(a["jobs"], 25);
        let (_, b) = post("/api/workloads/generate", spec).await;
        assert_eq!(a["yaml"], b["yaml"]);
        assert!(a["yaml"].as_str().unwrap().contains("tenant-"));
        let (s, v) = post(
            "/api/workloads/generate",
            json!({"jobs": 0, "arrival_rate_per_sec": 1.0, "median_runtime_secs": 1.0}),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        let (s, _) = call(
            Method::POST,
            "/api/workloads/generate",
            None,
            Some(json!({})),
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn load_sweep_returns_rows_per_scheduler_and_load() {
        let (s, v) = post(
            "/api/load-sweep",
            json!({"config": "backfill_demo.yaml", "schedulers": ["fifo", "easy"],
                   "loads": [2.0, 1.0], "max_wait_p90_secs": 100000.0}),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["loads"], json!([1.0, 2.0]));
        assert_eq!(v["rows"].as_array().unwrap().len(), 4);
        assert_eq!(v["sustainable"].as_array().unwrap().len(), 2);
        assert_eq!(v["sustainable"][0]["sustainable_load"], 2.0);

        let (s, _) = post("/api/load-sweep", json!({"config": "nope.yaml"})).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn load_sweep_rejects_bad_input() {
        for body in [
            json!({"config": "backfill_demo.yaml", "loads": [0.0]}),
            json!({"config": "backfill_demo.yaml", "loads": []}),
            json!({"config": "backfill_demo.yaml", "schedulers": []}),
            json!({"config": "../x.yaml"}),
        ] {
            let (s, v) = post("/api/load-sweep", body.clone()).await;
            assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
        }
    }
}
