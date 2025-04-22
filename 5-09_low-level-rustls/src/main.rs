//! Rust 생태계에서 가장 권장되는 TLS 방식인 rustls 를 기반으로 Axum 서버를 HTTPS로 구동하는 저수준 예제.
//! native-tls나 openssl 기반 예제와는 달리, 완전히 Rust로 구현된 TLS 스택을 사용하는 것이 핵심.
//!

use axum::{extract::Request, routing::get, Router}; // Axum 라우터 및 요청 추출
use futures_util::pin_mut;
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo}; // hyper ↔ tokio 호환 어댑터
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::net::TcpListener;

// rustls 관련 모듈
use tokio_rustls::{
    rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
    rustls::ServerConfig,
    TlsAcceptor,
};

use tower_service::Service;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 로그 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // rustls 기반 TLS 설정을 불러옴
    let rustls_config = rustls_server_config(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("key.pem"), // 개인키 경로
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("self_signed_certs")
            .join("cert.pem"), // 인증서 경로
    );

    let tls_acceptor = TlsAcceptor::from(rustls_config);

    // 바인딩 주소 (IPv6 localhost)
    let bind = "[::1]:3000";
    let tcp_listener = TcpListener::bind(bind).await.unwrap();

    info!("HTTPS server listening on {bind}. To contact curl -k https://localhost:3000");

    // 간단한 라우팅: GET / 요청을 처리
    let app = Router::new().route("/", get(handler));

    pin_mut!(tcp_listener);

    // 무한 루프: TLS 서버 동작
    loop {
        let tower_service = app.clone(); // tower 기반 앱 복제
        let tls_acceptor = tls_acceptor.clone();

        // TCP 연결 수락
        let (cnx, addr) = tcp_listener.accept().await.unwrap();

        // 연결마다 새로운 비동기 task 처리
        tokio::spawn(async move {
            // TLS 핸드셰이크 수행
            let Ok(stream) = tls_acceptor.accept(cnx).await else {
                error!("error during tls handshake connection from {}", addr);
                return;
            };

            // tokio ↔ hyper 변환
            let stream = TokioIo::new(stream);

            // hyper Service → tower Service 연결
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.
                tower_service.clone().call(request)
            });

            // HTTP1/2 + WebSocket 핸들링
            let ret = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(stream, hyper_service)
                .await;

            if let Err(err) = ret {
                warn!("error serving connection from {}: {}", addr, err);
            }
        });
    }
}

// 기본 핸들러: GET / 요청 → "Hello, World!"
async fn handler() -> &'static str {
    "Hello, World!"
}

// rustls 기반 서버 설정 함수
fn rustls_server_config(key: impl AsRef<Path>, cert: impl AsRef<Path>) -> Arc<ServerConfig> {
    // 개인키 로드 (.pem → PKCS#8 or RSA)
    let key = PrivateKeyDer::from_pem_file(key).unwrap();

    // 인증서 여러 개 로딩 (체인 가능)
    let certs = CertificateDer::pem_file_iter(cert)
        .unwrap()
        .map(|cert| cert.unwrap())
        .collect();

    // 서버 설정 빌더: 클라이언트 인증 없음, 단일 인증서 사용
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("bad certificate/key");

    // ALPN: HTTP/2 및 HTTP/1.1 지원 설정
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Arc::new(config)
}

// 💡 핵심 개념 요약
// 	•	이 예제는 rustls를 직접 사용하여 HTTPS 서버를 구성합니다.
// 	•	인증서와 개인키는 .pem 형식으로 제공되며, 서버 시작 시 로딩됩니다.
// 	•	ALPN (Application-Layer Protocol Negotiation)을 통해 HTTP/2와 HTTP/1.1 모두 지원합니다.
// 	•	hyper_util의 auto::Builder를 통해 요청 처리 루프를 구성하고, WebSocket 업그레이드도 지원됩니다.
// 	•	axum::Router는 tower::Service로 동작하기 때문에 hyper와 통합이 가능합니다.

// ⸻

// 🧪 테스트 방법
//
// curl -k https://localhost:3000
// # 응답: Hello, World!
// # -k는 self-signed 인증서 사용 시 필요! (인증 무시).

// ⸻

// ✅ 실무에 적합한 이유
// 	•	rustls는 C 의존성이 없고 완전한 Rust 구현이므로 보안성 및 이식성이 뛰어남.
// 	•	TLS 설정을 매우 세밀하게 제어할 수 있어, mTLS (상호 인증)이나 ALPN, SNI 등도 확장 가능.
// 	•	native-tls, openssl 기반보다 가볍고 현대적인 방식.

// ⸻

// ❓ 언제 rustls가 필요한가?
// 	•	ALB 없이 Axum 서버가 HTTPS를 직접 받아야 할 때
// 	•	내부망이지만 암호화가 꼭 필요할 때
// 	•	클라이언트 인증서 기반 mTLS 통신이 필요할 때
// 	•	외부 로드 밸런서 없이 서버 하나로 HTTPS 종단점을 만들고 싶을 때
// 	•	Cloudflare나 Nginx 없이 Rust만으로 HTTPS 서버 구성하고 싶을 때

// ⸻

// 🔧 확장 아이디어
// 	•	ALPN 설정에 따라 HTTP/2 또는 HTTP/1.1 전용 서버로 분리
// 	•	클라이언트 인증 (mTLS) 적용
// 	•	SessionResumption, OCSP Stapling, SNI 설정
// 	•	rustls::ClientConfig를 활용한 클라이언트 구현도 가능
