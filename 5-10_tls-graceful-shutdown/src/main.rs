//! TLS 서버 구성 및 우아한 종료를 포함한 HTTPS Axum 예제
//! Axum + rustls 기반의 HTTPS 서버에 대한 graceful shutdown 처리와 함께,
//! HTTP 요청을 HTTPS로 자동 리디렉션하는 두 개의 서버를 동시에 실행하는 예제.

use axum::{
    handler::HandlerWithoutStateExt,
    http::{uri::Authority, StatusCode, Uri},
    response::Redirect,
    routing::get,
    BoxError, Router,
};
use axum_extra::extract::Host;
use axum_server::tls_rustls::RustlsConfig;
use std::{future::Future, net::SocketAddr, path::PathBuf, time::Duration};
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone, Copy)]
struct Ports {
    http: u16,  // 리디렉션용 HTTP 포트
    https: u16, // TLS 처리용 HTTPS 포트
}

#[tokio::main]
async fn main() {
    // 로그 초기화
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

    // TLS 서버의 종료 신호를 처리하기 위한 핸들 생성
    let handle = axum_server::Handle::new();

    // Ctrl+C 또는 SIGTERM 수신 시 호출될 종료 future 준비
    let shutdown_future = shutdown_signal(handle.clone());

    // 보조 서버: HTTP → HTTPS 리디렉션을 백그라운드로 실행
    tokio::spawn(redirect_http_to_https(ports, shutdown_future));

    // rustls 인증서 설정 (PEM 포맷 인증서 + 키)
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

    let app = Router::new().route("/", get(handler));

    // HTTPS 서버 구동
    let addr = SocketAddr::from(([127, 0, 0, 1], ports.https));
    tracing::debug!("listening on {addr}");

    axum_server::bind_rustls(addr, config)
        .handle(handle) // graceful shutdown 을 위한 핸들 연결
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// 종료 신호 수신 시 서버를 우아하게 종료하는 future
async fn shutdown_signal(handle: axum_server::Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    // 유닉스 기반 OS에서 SIGTERM 처리
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    // 윈도우에서는 SIGTERM 없음 → pending 처리
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // 어느 신호가 먼저 오든 실행됨
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Received termination signal shutting down");
    // 종료 요청: 10초 내 종료를 시도함
    handle.graceful_shutdown(Some(Duration::from_secs(10))); // 10 secs is how long docker will wait
                                                             // to force shutdown
}

// 기본 라우트 핸들러
async fn handler() -> &'static str {
    "Hello, World!"
}

// 보조 서버: HTTP 요청을 HTTPS로 리디렉션 처리
async fn redirect_http_to_https<F>(ports: Ports, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    // 요청 host 와 URI 를 기반으로 HTTPS 버전으로 변환
    fn make_https(host: &str, uri: Uri, https_port: u16) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        // path가 비어 있으면 "/"로 설정
        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        // 호스트에서 포트 제거 (e.g. localhost:7878 → localhost)
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

        parts.authority = Some(format!("{bare_host}:{https_port}").parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    // 리디렉션 처리 핸들러
    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(&host, uri, ports.https) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::warn!(%error, "failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], ports.http));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {addr}");

    axum::serve(listener, redirect.into_make_service())
        .with_graceful_shutdown(signal) // 종료 시 함께 멈추도록
        .await
        .unwrap();
}

// • axum_server::Handle을 이용한 우아한 종료(graceful shutdown)
// • HTTP → HTTPS 자동 리디렉션 서버 (/ 경로 기준)
// • Ctrl+C 또는 SIGTERM 종료 신호 처리

// ✅ 이 예제의 핵심 요약
// 	•	axum_server::Handle을 이용해 서버를 안전하게 종료할 수 있습니다 (Ctrl+C, SIGTERM)
// 	•	tokio::spawn()을 이용하여 보조 HTTP 서버를 띄우고 HTTPS로 리디렉션 처리
// 	•	HTTPS는 rustls를 사용하며, 인증서는 PEM 파일로 설정
// 	•	axum_server는 hyper + tokio_rustls를 감싼 Axum 친화적 TLS 서버 라이브러리

// ⸻

// 💡 실무에 응용할 수 있는 부분
// 	•	SIGTERM은 Docker, Kubernetes 환경에서 매우 중요 (graceful shutdown 필수)
// 	•	HTTP → HTTPS 리디렉션은 보안 설정에서 기본 중의 기본
// 	•	axum_server를 활용하면 rustls + graceful shutdown을 간단하게 통합할 수 있음

// ⸻

// 🧪 테스트 예시
// 	1.	cargo run -p example-tls-graceful-shutdown 실행
//
// 	2.	브라우저 또는 curl 요청:
//   curl -v http://localhost:7878
//   # → 301 리디렉션 → https://localhost:3000
//
//   curl -k https://localhost:3000
//   # → "Hello, World!"
//
// 	3.	Ctrl+C 누르면 10초 동안 graceful하게 종료됨
