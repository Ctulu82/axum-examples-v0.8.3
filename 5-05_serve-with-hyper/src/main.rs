//!
//! serve-with-hyper 예제는 Axum을 hyper의 로우레벨 API로 구동하는 고급 구성.
//!
//! [hyper-util] 크레이트는 고수준 유틸리티를 제공하기 위해 존재하지만 아직 개발 초기 단계에 있습니다.
//! [hyper-util]: https://crates.io/crates/hyper-util
//!
//!🧭 이 예제의 목적
//! Axum은 내부적으로 tower 기반의 서비스 구조를 사용하지만, 하부 네트워크 레이어는 보통 hyper가 처리.
//! 이 예제는 그 hyper를 직접 제어하여 Axum 앱을 구동하는 방식임.
//!
//! 🧩 두 개의 서버 실행
//! serve_plain()               → 3000 포트, 기본 Axum Router
//! serve_with_connect_info()   → 3001 포트, 요청자의 IP 주소를 추출
//!   •	둘 다 TcpListener + hyper::server + TokioExecutor 기반으로 직접 연결 처리
//!   •	tower_service.clone().call(request) 또는 .oneshot() 호출로 Axum 앱에 요청 전달

use std::convert::Infallible;
use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::{extract::Request, routing::get, Router};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server;
use tokio::net::TcpListener;
use tower::{Service, ServiceExt};

/// 🧵 main: 두 서버를 동시에 실행
#[tokio::main]
async fn main() {
    // 두 서버를 동시에 실행 (future join)
    tokio::join!(serve_plain(), serve_with_connect_info());
}

/// 🌐 serve_plain(): 일반적인 연결 처리 (포트 3000)
/// 🔍 포인트:
///   > hyper::server::conn::auto::Builder 사용: HTTP/1 + HTTP/2 자동 지원
///   > TokioExecutor: hyper가 내부적으로 tokio::spawn() 사용할 수 있게 함
///   > Router는 tower::Service이므로 .call() 가능
async fn serve_plain() {
    // Create a regular axum app.
    let app = Router::new().route("/", get(|| async { "Hello!" }));

    // Create a `TcpListener` using tokio.
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    // Continuously accept new connections.
    loop {
        // In this example we discard the remote address. See `fn serve_with_connect_info` for how
        // to expose that.
        let (socket, _remote_addr) = listener.accept().await.unwrap();

        // We don't need to call `poll_ready` because `Router` is always ready.
        let tower_service = app.clone(); // 클론해서 사용

        // Spawn a task to handle the connection. That way we can handle multiple connections
        // concurrently.
        tokio::spawn(async move {
            // Hyper has its own `AsyncRead` and `AsyncWrite` traits and doesn't use tokio.
            // `TokioIo` converts between them.
            let socket = TokioIo::new(socket); // tokio <-> hyper 호환

            // Hyper also has its own `Service` trait and doesn't use tower. We can use
            // `hyper::service::service_fn` to create a hyper `Service` that calls our app through
            // `tower::Service::call`.
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                // We have to clone `tower_service` because hyper's `Service` uses `&self` whereas
                // tower's `Service` requires `&mut self`.
                //
                // We don't need to call `poll_ready` since `Router` is always ready.

                // tower Service → hyper Service 호출. (Axum의 Router는 tower::Service 이므로 직접 호출 가능)
                tower_service.clone().call(request)
            });

            // `server::conn::auto::Builder`: HTTP/1.1, HTTP/2 자동처리 지원.
            //
            // `TokioExecutor` tells hyper to use `tokio::spawn` to spawn tasks.
            if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                // `serve_connection_with_upgrades` is required for websockets. If you don't need
                // that you can use `serve_connection` instead.
                // WebSocket 과 같은 업그레이드 요청 처리 가능
                .serve_connection_with_upgrades(socket, hyper_service)
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

// Similar setup to `serve_plain` but captures the remote address and exposes it through the
// `ConnectInfo` extractor
/// 🌐 클라이언트 IP 추출 (포트 3001)
/// •	ConnectInfo<SocketAddr>를 통해 IP 추출 (ConnectInfo는 IP 추출용 Extractor)
/// •	into_make_service_with_connect_info()가 필수
async fn serve_with_connect_info() {
    let app = Router::new().route(
        "/",
        get(
            |ConnectInfo(remote_addr): ConnectInfo<SocketAddr>| async move {
                format!("Hello {remote_addr}")
            },
        ),
    );

    let mut make_service = app.into_make_service_with_connect_info::<SocketAddr>();

    let listener = TcpListener::bind("0.0.0.0:3001").await.unwrap();

    loop {
        let (socket, remote_addr) = listener.accept().await.unwrap();

        // We don't need to call `poll_ready` because `IntoMakeServiceWithConnectInfo` is always
        // ready.
        let tower_service = unwrap_infallible(make_service.call(remote_addr).await);

        tokio::spawn(async move {
            // tokio 소켓을 hyper에서 사용할 수 있게 래핑
            let socket = TokioIo::new(socket);

            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                tower_service.clone().oneshot(request)
            });

            if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
                // WebSocket 과 같은 업그레이드 요청 처리 가능
                .serve_connection_with_upgrades(socket, hyper_service)
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

// 타입 안정성을 위한 보조
fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => match err {},
    }
}

// 🧠 언제 이런 구조를 사용할까?
//
//  > 완전한 서버 제어가 필요한 경우: 직접 TCP 소켓 관리, 커스텀 프로토콜 혼합
//  > 기존 시스템이 hyper 기반일 때: tower 를 직접 끼워 넣기
//  > low-level control 이 필요함.
//  > HTTP/1, HTTP/2 자동 선택 필요: auto::Builder 사용.

// 🧪 테스트 예시
//
// curl http://localhost:3000
// # → Hello!
//
// curl http://localhost:3001
// # → Hello 127.0.0.1:xxxxx

// 📜 정리
// 이 예제는 Axum을 완전히 `커스텀 서버 레벨로 탈피`해서 제어하고자 할 때 아주 유용
// 이걸 기반으로 WebSocket 서버, mTLS 클라이언트 인증 서버, 게임 서버 게이트웨이 같은 고급 기능으로 확장 가능
