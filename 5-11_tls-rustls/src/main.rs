//! TLS 설정 기반 Axum HTTPS 서버 예제
//! HTTP 요청을 HTTPS로 리디렉션 처리
//!
//! 이전의 tls-graceful-shutdown 예제보다 더 단순화된 버전.
//! axum_server::bind_rustls를 이용한 HTTPS 서버 설정과, 보조 HTTP 서버에서 HTTPS로 리디렉션 처리만을 담당.

// 미사용 경고를 무시함
#![allow(unused_imports)]

use axum::{
    handler::HandlerWithoutStateExt,
    http::{uri::Authority, StatusCode, Uri},
    response::Redirect,
    routing::get,
    BoxError, Router,
};
use axum_extra::extract::Host; // Host 헤더를 추출해 실제 요청 호스트 확인.
use axum_server::tls_rustls::RustlsConfig;
use std::{net::SocketAddr, path::PathBuf};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ports {
    http: u16,  // 리디렉션용 HTTP 포트
    https: u16, // 메인 HTTPS 포트
}

#[tokio::main]
async fn main() {
    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let ports = Ports {
        http: 7878,
        https: 3000,
    };

    // 선택적 리디렉션 HTTP 서버 실행 (HTTP → HTTPS)
    // HTTP 포트(7878)에서 들어온 요청을 HTTPS(3000)로 리다이렉션
    tokio::spawn(redirect_http_to_https(ports));

    // rustls 인증서 및 개인키 설정
    let config = RustlsConfig::from_pem_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"),
    )
    .await
    .unwrap();

    // 라우터 설정: GET /
    let app = Router::new().route("/", get(handler));

    // HTTPS 서버 구동
    let addr = SocketAddr::from(([127, 0, 0, 1], ports.https));
    tracing::debug!("listening on {}", addr);

    // HTTPS 서버를 rustls 인증서 기반으로 실행.
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[allow(dead_code)]
async fn handler() -> &'static str {
    "Hello, World!"
}

#[allow(dead_code)]
async fn redirect_http_to_https(ports: Ports) {
    // 주어진 host/uri 조합을 HTTPS로 변경하는 함수
    // 요청 URI를 .scheme = https, .authority = hostname:port 으로 바꿔줌.
    fn make_https(host: &str, uri: Uri, https_port: u16) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        // 경로가 없다면 "/"로 기본 설정
        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        // 호스트에서 포트를 제거하여 정제된 hostname 추출
        let authority: Authority = host.parse()?;
        let bare_host = match authority.port() {
            Some(port_struct) => authority
                .as_str()
                .strip_suffix(port_struct.as_str())
                .unwrap()
                .strip_suffix(':')
                .unwrap(), // if authority.port() is Some(port) then we can be sure authority ends with :{port}
            None => authority.as_str(),
        };

        // 새 authority 설정: hostname:HTTPS_PORT
        parts.authority = Some(format!("{bare_host}:{https_port}").parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    // 리디렉션 라우터 설정
    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(&host, uri, ports.https) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())), // 301 리디렉션
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    // HTTP 서버 바인딩 및 실행
    let addr = SocketAddr::from(([127, 0, 0, 1], ports.http));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, redirect.into_make_service())
        .await
        .unwrap();
}

// 🧪 테스트 흐름
// # HTTP 요청 → HTTPS로 리디렉션 (브라우저도 가능)
// curl -v http://localhost:7878
// # → 301 Moved Permanently → Location: https://localhost:3000

// # HTTPS 요청 → 정상 응답
// curl -k https://localhost:3000
// # → Hello, World!

// `tls-rustls` 와. `tls-graceful-shutdown` 의 차이점
//
// `tls-graceful-shutdown`
//  - Graceful Shutdown: O (Ctrl+C 처리)
//  - Signal 처리: O
//  - 구조: 실전용
//
// `tls-graceful-shutdown`
//  - Graceful Shutdown: X
//  - Signal 처리: X
//  - 구조: 단순화된 예시
