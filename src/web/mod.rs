//! Embedded web server for interactive visualization.
//!
//! This module provides a local HTTP server that serves the built
//! React application and exposes the analysis data via a JSON API.
//!
//! - The analysis is serialized **once** at startup and served from a
//!   shared buffer; responses are gzip-compressed when the client accepts it.
//! - Hashed Vite assets (`/assets/*`) are served with immutable caching.
//! - If the port is busy, the next ports are tried.
//! - Ctrl+C shuts the server down gracefully.
//!
//! ## Security
//!
//! The server only listens on `127.0.0.1`, but that alone does not stop a
//! malicious website from reading `/api/data` (which contains absolute
//! paths and the project structure) through **DNS rebinding**. Every request
//! must therefore carry a `Host` header naming the loopback address and the
//! bound port; anything else is rejected with `421 Misdirected Request`.
//! Responses carry a strict Content-Security-Policy, `nosniff`,
//! `frame-ancestors 'none'` and `Cross-Origin-Resource-Policy: same-origin`.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::{header, HeaderName, HeaderValue, Response, StatusCode, Uri},
    middleware::{self, Next},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use tower_http::compression::CompressionLayer;

use crate::output::OutputSchema;

/// Embedded web assets from the built React application.
///
/// Built with `npm run build` in `web/`. When the folder is missing at
/// compile time the binary still builds and serves a short help page.
#[derive(RustEmbed)]
#[folder = "web/dist/"]
#[allow_missing = true]
struct WebAssets;

/// Number of consecutive ports tried when the requested one is busy.
const PORT_ATTEMPTS: u16 = 10;

/// Content-Security-Policy for every response.
///
/// React Flow positions nodes with inline styles and html-to-image exports
/// through `data:`/`blob:` URLs; everything else is same-origin only.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; worker-src 'self' blob:; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; \
     connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// Security headers attached to every response.
const SECURITY_HEADERS: [(&str, &str); 5] = [
    ("content-security-policy", CONTENT_SECURITY_POLICY),
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "DENY"),
    ("referrer-policy", "no-referrer"),
    ("cross-origin-resource-policy", "same-origin"),
];

/// Options for [`serve`].
#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    /// Preferred port.
    pub port: u16,
    /// Open the default browser once listening.
    pub open_browser: bool,
    /// Suppress status output.
    pub quiet: bool,
}

/// Starts the embedded web server and (optionally) opens the browser.
///
/// # Arguments
///
/// * `data` - The analysis output to serve via the API
/// * `options` - Port and browser behaviour
///
/// # Errors
///
/// Returns an error if no port could be bound or the server fails.
pub async fn serve(data: OutputSchema, options: ServeOptions) -> Result<()> {
    let payload = Bytes::from(serde_json::to_vec(&data).context("Failed to serialize analysis")?);
    drop(data);

    let listener = bind(options.port).await?;
    let addr = listener.local_addr()?;

    let app = Router::new()
        .route("/api/data", get(api_data))
        .fallback(static_handler)
        .with_state(payload)
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn_with_state(addr.port(), guard));
    let url = format!("http://{addr}");

    if !options.quiet {
        if addr.port() != options.port {
            eprintln!("Port {} is busy, using {}", options.port, addr.port());
        }
        if WebAssets::get("index.html").is_none() {
            eprintln!("warning: web UI assets were not embedded; run `npm ci && npm run build` in web/ and rebuild");
        }
        eprintln!("sass-dep visualizer running at {url} (Ctrl+C to stop)");
    }

    if options.open_browser {
        if let Err(error) = open::that(&url) {
            eprintln!(
                "warning: could not open browser automatically ({error}); open {url} manually"
            );
        }
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("Server error")
}

/// Rejects requests whose `Host` is not this server and adds security headers.
async fn guard(State(port): State<u16>, request: Request, next: Next) -> Response<Body> {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if !is_loopback_host(host, port) {
        return Response::builder()
            .status(StatusCode::MISDIRECTED_REQUEST)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(
                "Unknown host. Open the URL printed by sass-dep.",
            ))
            .expect("static response is valid");
    }

    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

/// Returns `true` if `host` is `127.0.0.1`, `localhost` or `[::1]` with exactly `port`.
fn is_loopback_host(host: &str, port: u16) -> bool {
    let Some((name, host_port)) = host.rsplit_once(':') else {
        return false;
    };
    host_port.parse::<u16>().is_ok_and(|p| p == port)
        && ["127.0.0.1", "localhost", "[::1]"]
            .iter()
            .any(|allowed| name.eq_ignore_ascii_case(allowed))
}

/// Binds to `port`, falling back to the next free ports.
async fn bind(port: u16) -> Result<tokio::net::TcpListener> {
    let mut last_error = None;
    for candidate in (0..PORT_ATTEMPTS).filter_map(|offset| port.checked_add(offset)) {
        match tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], candidate))).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => last_error = Some(error),
            Err(error) => {
                return Err(error).with_context(|| format!("Failed to bind to port {candidate}"))
            }
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("no port available"))).with_context(
        || {
            format!(
                "Ports {port}-{} are all in use",
                port.saturating_add(PORT_ATTEMPTS - 1)
            )
        },
    )
}

/// Serves the pre-serialized analysis JSON.
async fn api_data(State(payload): State<Bytes>) -> Response<Body> {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(payload))
        .expect("static response is valid")
}

/// Serves embedded static files, falling back to `index.html` for SPA routes.
async fn static_handler(uri: Uri) -> Response<Body> {
    let path = match uri.path().trim_start_matches('/') {
        "" => "index.html",
        path => path,
    };

    if let Some(file) = WebAssets::get(path) {
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, HeaderValue::from_static(cache))
            .body(Body::from(file.data))
            .expect("static response is valid");
    }

    if path == "index.html" || !path.contains('.') {
        let body = match WebAssets::get("index.html") {
            Some(index) => Body::from(index.data),
            None => Body::from(MISSING_UI_PAGE),
        };
        return Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(body)
            .expect("static response is valid");
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .expect("static response is valid")
}

/// Shown when the binary was built without the web UI.
const MISSING_UI_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>sass-dep</title></head>
<body style="font-family: system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; line-height: 1.5">
<h1>sass-dep</h1>
<p>This binary was built without the web visualizer. Build it and reinstall:</p>
<pre>cd web &amp;&amp; npm ci &amp;&amp; npm run build &amp;&amp; cd .. &amp;&amp; cargo install --path .</pre>
<p>The raw analysis is available at <a href="/api/data">/api/data</a>.</p>
</body></html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist() {
        assert!(is_loopback_host("127.0.0.1:3000", 3000));
        assert!(is_loopback_host("LOCALHOST:3000", 3000));
        assert!(is_loopback_host("[::1]:3000", 3000));

        assert!(!is_loopback_host("127.0.0.1:3001", 3000));
        assert!(!is_loopback_host("127.0.0.1", 3000));
        assert!(!is_loopback_host("evil.example:3000", 3000));
        assert!(!is_loopback_host("localhost.evil.example:3000", 3000));
        assert!(!is_loopback_host("127.0.0.1.nip.io:3000", 3000));
        assert!(!is_loopback_host("", 3000));
        assert!(!is_loopback_host("127.0.0.1:03000x", 3000));
    }
}
