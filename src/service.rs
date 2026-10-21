//! Authenticated loopback HTTP adapter. No client-selected repository configuration.
use crate::{
    adapter::{self, Error, Worker},
    application::Application,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore};

#[derive(Clone)]
struct Service {
    worker: Worker,
    token: Arc<String>,
    host: Arc<String>,
    requests: Arc<Semaphore>,
}
fn error(status: StatusCode, code: &str) -> Response {
    (
        status,
        axum::Json(json!({"api_version":1,"error":{"code":code}})),
    )
        .into_response()
}
fn domain_error(e: Error) -> Response {
    let status = match e {
        Error::InvalidArguments => StatusCode::BAD_REQUEST,
        Error::QueryFailed | Error::GitUnavailable => StatusCode::UNPROCESSABLE_ENTITY,
        Error::ResponseLimit => StatusCode::PAYLOAD_TOO_LARGE,
        Error::Busy => StatusCode::SERVICE_UNAVAILABLE,
        Error::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        axum::Json(json!({"api_version":1,"error":{"code":e,"message":e.to_string()}})),
    )
        .into_response()
}
fn exactly(headers: &axum::http::HeaderMap, key: header::HeaderName, expected: &str) -> bool {
    let mut all = headers.get_all(key).iter();
    all.next()
        .is_some_and(|v| v.as_bytes() == expected.as_bytes())
        && all.next().is_none()
}
fn authorized(headers: &axum::http::HeaderMap, token: &str) -> bool {
    // Fixed-size digest comparison keeps token-byte position out of comparison timing.
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    let expected = format!("Bearer {token}");
    let a = blake3::hash(value.as_bytes());
    let b = blake3::hash(expected.as_bytes());
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0u8, |d, (a, b)| d | (a ^ b))
        == 0
}
async fn handler(State(s): State<Service>, request: Request) -> Response {
    let mut response = handle(s, request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}
async fn handle(s: Service, request: Request) -> Response {
    if !exactly(request.headers(), header::HOST, &s.host)
        || request.headers().contains_key(header::ORIGIN)
    {
        return error(StatusCode::FORBIDDEN, "unsafe_authority");
    }
    if request.uri().query().is_some() {
        return error(StatusCode::BAD_REQUEST, "query_not_supported");
    }
    let path = request.uri().path();
    if path == "/health" {
        return if request.method() == "GET" {
            axum::Json(json!({"api_version":1,"status":"ok"})).into_response()
        } else {
            error(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
        };
    }
    if !authorized(request.headers(), &s.token) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    let name = match path {
        "/v1/search" => "search_codebase",
        "/v1/symbol" => "find_symbol",
        "/v1/neighborhood" => "get_dependency_neighborhood",
        "/v1/impact" => "get_change_impact",
        "/v1/context" => "build_task_context",
        _ => return error(StatusCode::NOT_FOUND, "not_found"),
    };
    if request.method() != "POST" {
        return error(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
    }
    if !exactly(request.headers(), header::CONTENT_TYPE, "application/json") {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "expected_application_json",
        );
    }
    let Ok(_request_permit) = s.requests.try_acquire() else {
        return domain_error(Error::Busy);
    };
    let body = match tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(request.into_body(), adapter::MAX_REQUEST),
    )
    .await
    {
        Err(_) => return error(StatusCode::REQUEST_TIMEOUT, "body_timeout"),
        Ok(Err(_)) => return error(StatusCode::PAYLOAD_TOO_LARGE, "body_limit"),
        Ok(Ok(bytes)) => bytes,
    };
    let r = match adapter::Request::parse(name, &body) {
        Ok(r) => r,
        Err(e) => return domain_error(e),
    };
    match tokio::time::timeout(
        Duration::from_secs(30),
        s.worker.run(r, |reply| {
            serde_json::to_vec(&reply).map_err(|_| Error::Internal)
        }),
    )
    .await
    {
        Err(_) => error(StatusCode::GATEWAY_TIMEOUT, "query_timeout"),
        Ok(Err(e)) => domain_error(e),
        Ok(Ok(bytes)) => (
            [(header::CONTENT_TYPE, "application/json")],
            Body::from(bytes),
        )
            .into_response(),
    }
}
pub fn validate_token(token: &str) -> Result<(), &'static str> {
    // Require a generated 256-bit hex token (32 random bytes). Length alone
    // cannot establish entropy; launching user remains responsible for generation.
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(
            "CONTEXT_ENGINE_SERVE_TOKEN must contain 64 hexadecimal characters (32 random bytes)",
        );
    }
    Ok(())
}
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
pub async fn run(
    app: Application,
    listen: SocketAddr,
    token: String,
) -> crate::application::Result<()> {
    if !listen.ip().is_loopback() {
        return Err("listen address must be a loopback IP".into());
    }
    validate_token(&token)?;
    let listener = TcpListener::bind(listen).await?;
    let address = listener.local_addr()?;
    let worker = Worker::new(app);
    let state = Service {
        worker: worker.clone(),
        token: Arc::new(token),
        host: Arc::new(address.to_string()),
        requests: Arc::new(Semaphore::new(16)),
    };
    let router = Router::new().fallback(any(handler)).with_state(state);
    eprintln!("ctx: listening http://{address}");
    let listener = LimitedListener {
        listener,
        connections: Arc::new(Semaphore::new(32)),
    };
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, router).with_graceful_shutdown(async {
        let _ = stopped.await;
    });
    // Stop accepting, drain sockets for five seconds, then close socket futures.
    // Blocking jobs retain admission and are drained separately without a deadline.
    let server = std::future::IntoFuture::into_future(server);
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result?,
        _ = shutdown() => { let _ = stop.send(()); let _ = tokio::time::timeout(Duration::from_secs(5), &mut server).await; }
    }
    worker.drain().await;
    Ok(())
}

// Bound live HTTP parsers/connections as well as bodies and blocking work. Idle
// clients cannot retain a slot indefinitely: every connection lives at most 45s.
struct LimitedListener {
    listener: TcpListener,
    connections: Arc<Semaphore>,
}
struct Connection {
    stream: tokio::net::TcpStream,
    _permit: tokio::sync::OwnedSemaphorePermit,
    deadline: std::pin::Pin<Box<tokio::time::Sleep>>,
}
impl axum::serve::Listener for LimitedListener {
    type Io = Connection;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Connection, SocketAddr) {
        let permit = self
            .connections
            .clone()
            .acquire_owned()
            .await
            .expect("connection semaphore open");
        let (stream, address) = axum::serve::Listener::accept(&mut self.listener).await;
        (
            Connection {
                stream,
                _permit: permit,
                deadline: Box::pin(tokio::time::sleep(Duration::from_secs(45))),
            },
            address,
        )
    }
    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}
impl Connection {
    fn check_deadline(&mut self, cx: &mut std::task::Context<'_>) -> std::io::Result<()> {
        use std::future::Future;
        if self.deadline.as_mut().poll(cx).is_ready() {
            Err(std::io::ErrorKind::TimedOut.into())
        } else {
            Ok(())
        }
    }
}
impl tokio::io::AsyncRead for Connection {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.check_deadline(cx)?;
        std::pin::Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
impl tokio::io::AsyncWrite for Connection {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.check_deadline(cx)?;
        std::pin::Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.check_deadline(cx)?;
        std::pin::Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
