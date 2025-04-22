//! low-level-native-tls 예제와 비슷하지만, TLS 구현체로 OpenSSL을 직접 사용하는 구조.
//! tokio-openssl을 통해 OpenSSL + Axum + Hyper + Tokio를 직접 결합하는 방식으로 HTTPS 서버를 만드는 예제

// 주요 모듈 import
use axum::{http::Request, routing::get, Router}; // Axum의 기본 Router
use futures_util::pin_mut;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo}; // hyper ↔ tokio 변환용
use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod}; // OpenSSL 관련
use std::{path::PathBuf, pin::Pin};
use tokio::net::TcpListener;
use tokio_openssl::SslStream; // tokio에서 OpenSSL TLS 스트림 처리용
use tower::Service;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    // TLS 핸드셰이크용 SslAcceptor 설정 (OpenSSL 모던 버전 사용)
    let mut tls_builder = SslAcceptor::mozilla_modern_v5(SslMethod::tls()).unwrap();

    // 인증서(.pem) 파일 설정
    tls_builder
        .set_certificate_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("cert.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    // 개인 키(.pem) 파일 설정
    tls_builder
        .set_private_key_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("self_signed_certs")
                .join("key.pem"),
            SslFiletype::PEM,
        )
        .unwrap();

    // 키 유효성 검사
    tls_builder.check_private_key().unwrap();

    // TLS acceptor 완성
    let tls_acceptor = tls_builder.build();

    // 바인딩 주소 (IPv6 localhost)
    let bind = "[::1]:3000";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();

    info!("HTTPS server listening on {bind}. To contact curl -k https://localhost:3000");

    // 라우터 설정: GET / 요청만 허용
    let app = Router::new().route("/", get(handler));

    pin_mut!(tcp_listener);

    // 요청 루프 시작
    loop {
        let tower_service = app.clone();
        let tls_acceptor = tls_acceptor.clone();

        // TCP 연결 수락
        let (cnx, addr) = tcp_listener.accept().await.unwrap();

        tokio::spawn(async move {
            // OpenSSL 스트림 객체 생성
            let ssl = Ssl::new(tls_acceptor.context()).unwrap();
            let mut tls_stream = SslStream::new(ssl, cnx).unwrap();

            // TLS 핸드셰이크 (client hello 등 처리)
            if let Err(err) = SslStream::accept(Pin::new(&mut tls_stream)).await {
                error!(
                    "error during tls handshake connection from {}: {}",
                    addr, err
                );
                return;
            }

            // Tokio ↔ Hyper 호환 스트림으로 래핑
            let stream = TokioIo::new(tls_stream);

            // Hyper 서비스 생성 → 내부적으로 tower::Service 호출
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.
                tower_service.clone().call(request)
            });

            // HTTP1/HTTP2 + WebSocket 가능하도록 Hyper 연결 처리
            let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            if let Err(err) = ret {
                warn!("error serving connection from {}: {}", addr, err);
            }
        });
    }
}

// 기본 핸들러: GET / 요청에 대해 응답
async fn handler() -> &'static str {
    "Hello, World!"
}

// 🧠 이 예제의 핵심 포인트 요약
// 	•	openssl crate을 기반으로 HTTPS 서버 구성
// 	•	TLS 연결을 직접 수락하고 SslStream을 수동으로 구성함
// 	•	SslStream::accept()을 await으로 호출해 TLS 핸드셰이크 수행
// 	•	클라이언트가 curl 같은 프로그램으로 접근 시 -k 옵션(인증서 무시) 필요
// 	•	hyper_util을 통해 HTTP 1.x / 2.x 자동 지원 가능
// 	•	실무에서는 OpenSSL 기능을 활용해 mTLS(상호 인증) 같은 고급 기능으로 확장 가능

// ⸻

// 🧪 테스트 예시
// curl -k https://localhost:3000
// # 응답: Hello, World!

// •	-k는 self-signed 인증서이므로 TLS 인증을 무시하고 강제로 연결함

// ⸻

// 🔧 실무 활용 아이디어
// 	•	내부 전용 API 서버를 OpenSSL 기반으로 직접 호스팅하고 싶을 때
// 	•	mTLS 기반 인증 서버 구축
// 	•	클라이언트 인증서 기반 사용자 식별
// 	•	OpenSSL의 풍부한 옵션 활용 (세션 재사용, ALPN 등)

// ⸻

// 필요에 의한 확장 고려:
// 	•	cert.pem/key.pem 생성 명령어
// 	•	mTLS 인증서 검증까지 확장하는 방법
// 	•	rustls 기반으로의 대체 구현
