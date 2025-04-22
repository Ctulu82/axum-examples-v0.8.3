//! Run with
//!
//! ```not_rust
//! cargo run -p example-low-level-native-tls
//! ```

// 필요 모듈 import
use axum::{extract::Request, routing::get, Router}; // Axum 기본 라우터
use futures_util::pin_mut; // TcpListener를 고정시켜 사용할 때 필요
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo}; // tokio ↔ hyper 호환 어댑터
use std::path::PathBuf;
use tokio::net::TcpListener;

// native-tls를 tokio 기반으로 wrapping한 라이브러리
use tokio_native_tls::{
    native_tls::{Identity, Protocol, TlsAcceptor as NativeTlsAcceptor},
    TlsAcceptor,
};

use tower_service::Service;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_low_level_rustls=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // TLS 인증서 및 키 파일 로드
    let tls_acceptor = native_tls_acceptor(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"),
    );

    // native_tls → tokio_native_tls 로 변환
    let tls_acceptor = TlsAcceptor::from(tls_acceptor);

    // 리스닝 주소 지정
    let bind = "[::1]:3000";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();

    info!("HTTPS server listening on {bind}. To contact curl -k https://localhost:3000");

    // 기본 라우터 생성
    let app = Router::new().route("/", get(handler));

    pin_mut!(tcp_listener); // TcpListener는 반복적으로 사용할 수 있도록 pin 처리

    // 메인 이벤트 루프: 연결 수락 반복
    loop {
        let tower_service = app.clone();
        let tls_acceptor = tls_acceptor.clone();

        // 새로운 TCP 연결 대기
        let (cnx, addr) = tcp_listener.accept().await.unwrap();

        // 각 연결을 비동기 task로 처리
        tokio::spawn(async move {
            // TLS 핸드셰이크 수행
            let Ok(stream) = tls_acceptor.accept(cnx).await else {
                error!("error during tls handshake connection from {}", addr);
                return;
            };

            // Hyper ↔ tokio 호환을 위한 래핑
            let stream = TokioIo::new(stream);

            // hyper::service::service_fn 으로 hyper Service 생성
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.
                tower_service.clone().call(request)
            });

            // HTTP 1 or 2 연결 처리 + WebSocket 업그레이드 가능
            let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            // 오류 처리
            if let Err(err) = ret {
                warn!("error serving connection from {addr}: {err}");
            }
        });
    }
}

// 기본 핸들러: GET / → “Hello, World!” 응답
async fn handler() -> &'static str {
    "Hello, World!"
}

// 인증서와 키 파일을 사용해 native TLS acceptor 생성
fn native_tls_acceptor(key_file: PathBuf, cert_file: PathBuf) -> NativeTlsAcceptor {
    let key_pem = std::fs::read_to_string(&key_file).unwrap();
    let cert_pem = std::fs::read_to_string(&cert_file).unwrap();

    // PEM 포맷의 키/인증서를 Identity로 변환
    let id = Identity::from_pkcs8(cert_pem.as_bytes(), key_pem.as_bytes()).unwrap();

    // TLS 버전 제한 및 빌더 생성
    NativeTlsAcceptor::builder(id)
        // let's be modern
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .unwrap()
}

// Axum을 직접 TLS 계층 위에 올리는 구조를 보여주는 예제.
// 주요 특징은 Rust에서 TLS 핸드셰이크를 직접 처리하며, native-tls를 사용해 HTTPS를 구현한다는 점.

// ⸻

// 🧭 흐름 요약
// 	1.	cert.pem, key.pem 파일을 읽어와서 TLS 설정을 초기화합니다.
// 	2.	TcpListener가 [::1]:3000 (IPv6 localhost) 포트에서 연결을 대기합니다.
// 	3.	클라이언트가 연결되면 TLS 핸드셰이크를 수행하고,
// 	4.	그 위에 hyper 서버를 직접 구동해 Axum의 라우터로 요청을 처리합니다.
// 	5.	TLS 종료와 HTTP 요청 처리를 직접 분리 구현한 구조입니다.

// ⸻

// 💡 특징 및 장점
// 	•	native-tls를 통해 Windows/macOS/Linux 환경에서 기본 시스템 TLS 라이브러리를 사용할 수 있습니다.
// 	•	axum::serve()를 사용하지 않고, TCP + TLS → hyper → tower → axum으로 직접 체인을 구성합니다.
// 	•	WebSocket이나 mTLS 인증, custom handshake 등 확장하기에 좋은 구조입니다.
// 	•	실무에서는 nginx나 traefik 없이 직접 HTTPS 서버를 띄우고 싶을 때 유용합니다.

// ⸻

// 확장
// cert.pem, key.pem을 생성하는 방법
// 이 구조를 rustls 기반으로 바꾸는 방법
