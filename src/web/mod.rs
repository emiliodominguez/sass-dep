//! Embedded web server for interactive visualization.
//!
//! This module provides a local HTTP server that serves the built
//! React application and exposes the analysis data via a JSON API.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use rust_embed::RustEmbed;

use crate::output::OutputSchema;

/// Embedded web assets from the built React application.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct WebAssets;

/// Application state shared across request handlers.
struct AppState {
    data: OutputSchema,
}

/// Starts the embedded web server and opens the browser.
///
/// # Arguments
///
/// * `data` - The analysis output to serve via the API
/// * `port` - The port to listen on
///
/// # Errors
///
/// Returns an error if:
/// - The server fails to bind to the specified port
/// - The browser fails to open
pub async fn serve(data: OutputSchema, port: u16) -> Result<()> {
    let state = Arc::new(AppState { data });

    let app = Router::new()
        .route("/api/data", get(api_data))
        .fallback(static_handler)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://localhost:{}", port);

    eprintln!("Starting web visualization server...");
    eprintln!("Opening browser at {}", url);
    eprintln!("Press Ctrl+C to stop the server");

    // Open browser (best effort - don't fail if it doesn't work)
    if let Err(e) = open::that(&url) {
        eprintln!("Warning: Could not open browser automatically: {}", e);
        eprintln!("Please open {} manually", url);
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to port {}", port))?;

    axum::serve(listener, app)
        .await
        .context("Server error")?;

    Ok(())
}

/// Handler for the API data endpoint.
async fn api_data(State(state): State<Arc<AppState>>) -> Json<OutputSchema> {
    Json(state.data.clone())
}

/// Handler for serving static files from embedded assets.
async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Default to index.html for root path
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            // For SPA routing, serve index.html for non-file paths
            if !path.contains('.') {
                if let Some(content) = WebAssets::get("index.html") {
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "text/html")
                        .body(Body::from(content.data.into_owned()))
                        .unwrap();
                }
            }
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()
        }
    }
}
