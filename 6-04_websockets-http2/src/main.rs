//! websockets-http2

use axum::{
    extract::{
        ws::{self, WebSocketUpgrade}, // 웹소켓 업그레이드 요청 처리
        State,                        // Axum 상태 추출기
    },
    http::Version, // HTTP/1.1, HTTP/2 구분을 위한 타입
    routing::any,  // GET, POST 등 상관없이 수락하는 라우팅
    Router,
};

use axum_server::tls_rustls::RustlsConfig; // HTTPS 설정을 위한 Rustls 모듈
use std::{net::SocketAddr, path::PathBuf}; // 주소, 경로 등 OS 타입
use tokio::sync::broadcast; // 비동기 브로드캐스트 채널
use tower_http::services::ServeDir; // 정적 파일 제공 (HTML, JS 등)
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt}; // 로그 추적

/// 🚀 main() 함수

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    // configure certificate and private key used by https
    // Rustls 기반 HTTPS 설정 (cert.pem, key.pem)
    // 프로젝트 루트 기준으로 assets/와 self_signed_certs/ 디렉토리 경로 계산
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

    // --- 🌐 Axum 앱 구성

    // build our application with some routes and a broadcast channel
    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/ws", any(ws_handler))
        .with_state(broadcast::channel::<String>(16).0);

    // 🧵 서버 실행
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    let mut server = axum_server::bind_rustls(addr, config);

    // IMPORTANT: This is required to advertise our support for HTTP/2 websockets to the client.
    // If you use axum::serve, it is enabled by default.
    // HTTP/2 기반 웹소켓 지원 명시
    server.http_builder().http2().enable_connect_protocol();

    server.serve(app.into_make_service()).await.unwrap();
}

/// 🧠 ws_handler() — WebSocket 처리 진입점

async fn ws_handler(
    ws: WebSocketUpgrade,
    version: Version,
    State(sender): State<broadcast::Sender<String>>,
) -> axum::response::Response {
    tracing::debug!("accepted a WebSocket using {version:?}");
    let mut receiver = sender.subscribe();
    ws.on_upgrade(|mut ws| async move {
        // 🔁 WebSocket 이벤트 루프 (양방향 처리)
        loop {
            tokio::select! {
                // Since `ws` is a `Stream`, it is by nature cancel-safe.
                // 클라이언트 → 서버 메시지 수신
                res = ws.recv() => {
                    match res {
                        Some(Ok(ws::Message::Text(s))) => {
                            let _ = sender.send(s.to_string()); // 다른 클라이언트에게 전송
                        }
                        Some(Ok(_)) => {}   // Binary, Ping 등은 무시
                        Some(Err(e)) => tracing::debug!("client disconnected abruptly: {e}"),
                        None => break,
                    }
                }

                // Tokio guarantees that `broadcast::Receiver::recv` is cancel-safe.
                // 서버 → 클라이언트 메시지 송신
                res = receiver.recv() => {
                    match res {
                        Ok(msg) => if let Err(e) = ws.send(ws::Message::Text(msg.into())).await {
                            tracing::debug!("client disconnected abruptly: {e}");
                        }
                        Err(_) => continue,
                    }
                }
            }
        }
    })
}

// 🚀 전체 테스트 흐름
//
// # 1. 프로젝트 루트에서 웹소켓 HTTPS 서버 실행
// cargo run -p example-websockets-http2
//
// # 2. 이후 두 개의 브라우저 창에서 다음 주소로 접근:
// https://localhost:3000/
//  •	입력창에 메시지를 작성하고 [Send]를 누르면
// 	•	WebSocket을 통해 서버에 전달되고
// 	•	서버는 broadcast::channel을 통해 모든 클라이언트에게 메시지를 전송
// 	•	두 창에서 실시간 메시지 수신 가능 🎉
