//! Same-origin HTTP transport for the single-owner Docker deployment.

use crate::{api_state::Api, domain::DomainCommand, infra::source, media, playback_log, services};
use base64::{engine::general_purpose::STANDARD, Engine};
use http::{header, HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::{
    net::TcpListener,
    sync::{Mutex, Semaphore},
};

const MEDIA_PREFIX: &str = "/media";
type WebResponse = Response<Full<Bytes>>;

struct WebState {
    api: Api,
    account: Mutex<()>,
    password: String,
    assets: PathBuf,
    data: PathBuf,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !crate::network_policy::strict_security() {
        return Err("Web deployment requires strict network security".into());
    }
    let password = std::env::var("HGHUBX_WEB_PASSWORD")?;
    if password.len() < 16 || !password.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("HGHUBX_WEB_PASSWORD must contain at least 16 printable ASCII characters without spaces".into());
    }
    let assets =
        PathBuf::from(std::env::var("HGHUBX_WEB_ASSETS").unwrap_or_else(|_| "dist".into()));
    tokio::fs::metadata(assets.join("index.html")).await?;
    let data = PathBuf::from(std::env::var("HGHUBX_WEB_DATA").unwrap_or_else(|_| "data".into()));
    tokio::fs::create_dir_all(&data).await?;
    source::bind(&data);
    playback_log::bind(&data);
    let api = Api::new();
    api.session.bind(data.clone());
    let state = Arc::new(WebState {
        api,
        account: Mutex::new(()),
        password: STANDARD.encode(format!("admin:{password}")),
        assets,
        data,
    });
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    let connections = Arc::new(Semaphore::new(128));
    eprintln!("HGhubX Web listening on port 8080");

    loop {
        let permit = connections.clone().acquire_owned().await?;
        let (socket, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let service = service_fn(move |request| {
                let state = state.clone();
                async move { Ok::<_, Infallible>(handle(request, &state).await) }
            });
            let _ = http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(Duration::from_secs(10))
                .serve_connection(TokioIo::new(socket), service)
                .await;
        });
    }
}

fn authorized(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .is_some_and(|(scheme, value)| {
            scheme.eq_ignore_ascii_case("Basic")
                && bool::from(value.as_bytes().ct_eq(expected.as_bytes()))
        })
}

async fn handle(request: Request<Incoming>, state: &WebState) -> WebResponse {
    let path = request.uri().path();
    if request.method() == Method::GET && path == "/healthz" {
        return plain(StatusCode::OK, "ok");
    }
    if request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|site| site == "cross-site" || site == "same-site")
        && (path.starts_with("/api/") || path.starts_with("/media/"))
    {
        return plain(StatusCode::FORBIDDEN, "same-origin requests only");
    }
    if !authorized(request.headers(), &state.password) {
        let mut response = plain(StatusCode::UNAUTHORIZED, "authentication required");
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"HGhubX\""),
        );
        return response;
    }

    if let Some(command) = path.strip_prefix("/api/") {
        if request.method() != Method::POST {
            return plain(StatusCode::METHOD_NOT_ALLOWED, "POST required");
        }
        // A cross-origin HTML form cannot send this header. No CORS is enabled.
        if request.headers().get("x-hghubx-web") != Some(&HeaderValue::from_static("1")) {
            return plain(StatusCode::FORBIDDEN, "missing application header");
        }
        let command = command.to_owned();
        let body = tokio::time::timeout(
            Duration::from_secs(10),
            Limited::new(request.into_body(), 1_048_576).collect(),
        )
        .await;
        let Ok(Ok(body)) = body else {
            return plain(StatusCode::BAD_REQUEST, "invalid request body");
        };
        let result = match serde_json::from_slice::<Value>(&body.to_bytes()) {
            Ok(args) => dispatch(state, &command, args).await,
            Err(_) => Err("请求格式无效".into()),
        };
        let (status, payload) = match result {
            Ok(data) => (StatusCode::OK, data),
            Err(error) => (StatusCode::BAD_REQUEST, json!({ "error": error })),
        };
        return response(status, "application/json", payload.to_string().into_bytes());
    }

    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return plain(StatusCode::METHOD_NOT_ALLOWED, "GET or HEAD required");
    }
    let mut result = if let Some(route) = path
        .strip_prefix(MEDIA_PREFIX)
        .filter(|route| route.starts_with('/'))
    {
        if !matches!(route, "/img" | "/hls" | "/seg") {
            return plain(StatusCode::NOT_FOUND, "not found");
        }
        let mut resource =
            media::handle(route, request.uri().query().unwrap_or(""), MEDIA_PREFIX).await;
        if route == "/seg" && resource.status() == StatusCode::OK {
            if let Some(range) = request.headers().get(header::RANGE) {
                apply_range(&mut resource, range);
            }
        }
        resource
            .headers_mut()
            .remove(header::ACCESS_CONTROL_EXPOSE_HEADERS);
        if let Some(cache) = resource.headers_mut().get_mut(header::CACHE_CONTROL) {
            if let Some(seconds) = cache
                .to_str()
                .ok()
                .and_then(|value| value.strip_prefix("public, max-age="))
            {
                *cache = HeaderValue::from_str(&format!("private, max-age={seconds}")).unwrap();
            }
        }
        resource.headers_mut().insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox; default-src 'none'"),
        );
        resource.headers_mut().insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
        let (parts, bytes) = resource.into_parts();
        Response::from_parts(parts, Full::new(Bytes::from(bytes)))
    } else {
        static_asset(&state.assets, path).await
    };
    result.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    if request.method() == Method::HEAD {
        *result.body_mut() = Full::new(Bytes::new());
    }
    result
}

fn parse_args<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|_| "请求参数无效".into())
}

async fn dispatch(state: &WebState, command: &str, args: Value) -> Result<Value, String> {
    match command {
        "data_call" => {
            let command: DomainCommand = parse_args(args["command"].clone())?;
            // Serialize account writes with source changes. Other requests may run concurrently.
            let _account = if matches!(
                &command,
                DomainCommand::Login { .. }
                    | DomainCommand::Register { .. }
                    | DomainCommand::Logout
            ) {
                Some(state.account.lock().await)
            } else {
                None
            };
            services::dispatch(&state.api, command).await
        }
        "gate_state" => services::to_json(&source::gate_state()),
        "gate_open" => {
            let address: String = parse_args(args["address"].clone())?;
            let normalized = source::normalize_door(&address);
            crate::network_policy::parse_target(&normalized)?;
            let _account = state.account.lock().await;
            let previous = source::base();
            let next = source::gate_open(&address)?;
            if previous != next.base {
                state.api.session.clear();
                media::clear_caches();
            }
            services::to_json(&next)
        }
        "gate_reset" => {
            let _account = state.account.lock().await;
            let next = source::gate_reset();
            state.api.session.clear();
            media::clear_caches();
            services::to_json(&next)
        }
        "cache_clear" => {
            media::clear_caches();
            Ok(Value::Null)
        }
        "net_diag" => Ok(crate::net::diagnose().await),
        "debug_log" => {
            let msg: String = parse_args(args["msg"].clone())?;
            crate::logging::append_debug(&state.data, &msg)?;
            Ok(Value::Null)
        }
        "playback_log_status" => services::to_json(&playback_log::playback_log_status()?),
        "playback_log_set" => services::to_json(&playback_log::playback_log_set(parse_args(
            args["enabled"].clone(),
        )?)?),
        "playback_log_write" => {
            playback_log::playback_log_write(parse_args(args["entries"].clone())?)?;
            Ok(Value::Null)
        }
        "playback_log_read" => Ok(json!(playback_log::playback_log_read()?)),
        _ => Err("未知 Web 命令".into()),
    }
}

async fn static_asset(root: &Path, path: &str) -> WebResponse {
    let file = match path {
        "/" => "index.html",
        "/logo.svg" => "logo.svg",
        _ => match path.strip_prefix("/assets/") {
            Some(name)
                if !name.starts_with('.')
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) =>
            {
                &path[1..]
            }
            _ => return plain(StatusCode::NOT_FOUND, "not found"),
        },
    };
    let mime = match Path::new(file).extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    };
    match tokio::fs::read(root.join(file)).await {
        Ok(bytes) => {
            let mut result = response(StatusCode::OK, mime, bytes);
            if file == "index.html" {
                result.headers_mut().insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static(
                        "frame-ancestors 'none'; base-uri 'none'; object-src 'none'",
                    ),
                );
            }
            result
        }
        Err(_) => plain(StatusCode::NOT_FOUND, "not found"),
    }
}

fn byte_range(range: &str, len: usize) -> Option<(usize, usize)> {
    let (start, end) = range.strip_prefix("bytes=")?.split_once('-')?;
    let last = len.checked_sub(1)?;
    if start.is_empty() {
        let suffix = end.parse::<usize>().ok()?.min(len);
        return (suffix != 0).then_some((len - suffix, last));
    }
    let start = start.parse::<usize>().ok()?;
    let end = if end.is_empty() {
        last
    } else {
        end.parse::<usize>().ok()?.min(last)
    };
    (start <= end).then_some((start, end))
}

fn apply_range(resource: &mut Response<Vec<u8>>, range: &HeaderValue) {
    let length = resource.body().len();
    let content_range = match range
        .to_str()
        .ok()
        .and_then(|range| byte_range(range, length))
    {
        Some((start, end)) => {
            *resource.status_mut() = StatusCode::PARTIAL_CONTENT;
            *resource.body_mut() = resource.body()[start..=end].to_vec();
            format!("bytes {start}-{end}/{length}")
        }
        None => {
            *resource.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            resource.body_mut().clear();
            format!("bytes */{length}")
        }
    };
    let size = resource.body().len().to_string();
    resource.headers_mut().insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&content_range).unwrap(),
    );
    resource.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&size).unwrap(),
    );
}

fn plain(status: StatusCode, text: &str) -> WebResponse {
    response(
        status,
        "text/plain; charset=utf-8",
        text.as_bytes().to_vec(),
    )
}

fn response(status: StatusCode, mime: &str, bytes: Vec<u8>) -> WebResponse {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Full::new(Bytes::from(bytes)))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_requires_matching_credentials() {
        let mut headers = HeaderMap::new();
        assert!(!authorized(&headers, "YWJj"));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("basic YWJj"),
        );
        assert!(authorized(&headers, "YWJj"));
        assert!(!authorized(&headers, "YWJk"));
    }

    #[test]
    fn single_ranges_include_both_endpoints() {
        assert_eq!(byte_range("bytes=2-4", 10), Some((2, 4)));
        assert_eq!(byte_range("bytes=2-", 10), Some((2, 9)));
        assert_eq!(byte_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(byte_range("bytes=10-", 10), None);
        assert_eq!(byte_range("bytes=0-1,3-4", 10), None);
        assert_eq!(byte_range("bytes=0-", 0), None);
    }
}
