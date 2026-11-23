// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

mod auth;
mod error;
mod presets;
mod profile_registry;
mod routes;
mod run_registry;
mod state;
mod twin_store;
mod ws_ticket;

use axum::routing::get;
use axum::{middleware, Router};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let state = AppState::from_env();

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

    let app = public_routes
        .merge(authenticated_routes)
        .merge(ws_routes)
        .merge(shim_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:8080";
    tracing::info!("zyvor-janus-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind 0.0.0.0:8080");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server error");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
