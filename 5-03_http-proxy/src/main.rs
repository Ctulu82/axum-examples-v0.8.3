//! 이 예제는 프록시 또는 네트워크 인프라 관련 서비스를 만들 경우 직접 통제할 수 있는 실전 스킬을 익히는 데에 유용.
//! 이 예제는 단순한 API 서버가 아니라, Proxy 서버, 특히 HTTP CONNECT 방식 터널링을 다룸.
//!
//! 📌 이 예제의 목적
//! curl -x 127.0.0.1:3000 https://tokio.rs 같은 요청을 받을 때,
//! 실제로 HTTPS 연결을 CONNECT 방식으로 중계(proxy)하는 예제임.
//!
//! 다른 터미널에서 테스트:
//! curl -v -x "127.0.0.1:3000" https://tokio.rs
//!
//! Example is based on <https://github.com/hyperium/hyper/blob/master/examples/http_proxy.rs>

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::upgrade::Upgraded;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tower::Service;
use tower::ServiceExt;

use hyper_util::rt::TokioIo;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 로그 초기화 (RUST_LOG=example-http-proxy=trace)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=trace,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 간단한 라우터: GET / 요청 시 Hello 응답
    let router_svc = Router::new().route("/", get(|| async { "Hello, World!" }));

    // tower service 함수 생성
    let tower_service = tower::service_fn(move |req: Request<_>| {
        let router_svc = router_svc.clone();
        let req = req.map(Body::new); // hyper용 요청 타입으로 변환

        async move {
            // CONNECT 요청이면 프록시 처리
            if req.method() == Method::CONNECT {
                proxy(req).await
            } else {
                // 그 외는 라우터로 처리
                router_svc.oneshot(req).await.map_err(|err| match err {})
            }
        }
    });

    // hyper 전용 service 로 감싸기
    let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
        tower_service.clone().call(request)
    });

    // 서버 리스너 시작
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    let listener = TcpListener::bind(addr).await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let hyper_service = hyper_service.clone();

        // 연결마다 새로운 task로 서비스 처리
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, hyper_service)
                .with_upgrades() // CONNECT 처리를 위해 필수
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

/// 🔌 proxy() 함수: CONNECT 처리
// CONNECT 요청 처리 → TCP 터널 생성
async fn proxy(req: Request) -> Result<Response, hyper::Error> {
    tracing::trace!(?req);

    // 요청 URI에서 호스트 주소 추출
    if let Some(host_addr) = req.uri().authority().map(|auth| auth.to_string()) {
        // 업그레이드 요청을 기다렸다가 → 업그레이드 완료되면 TCP 터널 생성
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(e) = tunnel(upgraded, host_addr).await {
                        tracing::warn!("server io error: {}", e);
                    }
                }
                Err(e) => tracing::warn!("upgrade error: {}", e),
            }
        });

        // 클라이언트에게는 빈 응답만 먼저 반환
        Ok(Response::new(Body::empty()))
    } else {
        tracing::warn!("CONNECT host is not socket addr: {:?}", req.uri());
        Ok((
            StatusCode::BAD_REQUEST,
            "CONNECT must be to a socket address",
        )
            .into_response())
    }
}

/// 🔄 tunnel(): TCP 터널링 처리
// 클라이언트와 원격 서버 간의 TCP 터널 처리
async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?; // 원격 서버 연결
    let mut upgraded = TokioIo::new(upgraded); // 클라이언트 스트림

    // 양방향 통신: 클라이언트 <-> 원격 서버
    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    tracing::debug!(
        "client wrote {} bytes and received {} bytes",
        from_client,
        from_server
    );

    Ok(())
}

// 🔁 실행 흐름 요약
// 	1.	클라이언트는 프록시 서버에 CONNECT 요청을 보냄
// 	2.	서버는 CONNECT 요청을 인식하고 proxy() 함수로 처리
// 	3.	hyper::upgrade::on()을 통해 TCP 레벨로 connection을 업그레이드
// 	4.	실제 원격 서버(tokio.rs:443)로 연결하여 터널을 생성
// 	5.	tokio::io::copy_bidirectional()로 터널 통신을 양방향 중계

// curl -v -x "127.0.0.1:3000" https://tokio.rs
//  > 이 명령은 프록시 서버를 경유하여 HTTPS 요청을 수행하는 구조임.
//  > 즉, curl이 직접 https://tokio.rs에 연결하는 대신,
//  > 먼저 프록시 서버(127.0.0.1:3000)에 CONNECT tokio.rs:443 요청을 보낸 다음,
//  > 프록시 서버를 통해 HTTPS 요청을 우회 중계하는 구조.
//  > [curl] → [프록시 서버: 127.0.0.1:3000] → [실제 대상: tokio.rs:443]

// 🧩 단계별 흐름 설명
// 🔹 1. curl 시작
// 	•	-x = proxy 설정 (--proxy)
//
// 🔹 2. curl → 프록시로 CONNECT 요청 전송
//  •	이건 “프록시야, 나 대신 tokio.rs:443 로 TCP 연결 좀 만들어줘” 라는 뜻
//  •	이건 일반적인 HTTP 요청이 아니라, HTTP CONNECT 메서드
//
// 🔹 3. 프록시 서버가 tokio.rs:443 에 TCP 연결 시도
//	•	예제의 proxy() 함수가 호출됨
//  •	내부적으로 TcpStream::connect("tokio.rs:443") 수행
//  •	성공하면: 클라이언트와 tokio.rs:443 간 양방향 터널 생성
//
// 🔹 4. 프록시가 HTTP/1.1 200 Connection established 응답
//    HTTP/1.1 200 Connection established  # curl은 이제부터 프록시를 통해서만 통신
//
// 🔹 5. curl → HTTPS 요청 전송 (터널 내부에서)
//    GET / HTTP/1.1    # curl이 TLS 핸드셰이크를 시작하고, HTTPS GET 요청을 보냄
//    Host: tokio.rs
//    User-Agent: curl/...# 프록시는 payload가 뭔지 전혀 알 수 없음 (암호화되어 있기 때문)
//
// 🔹 6. 프록시가 모든 데이터를 그대로 중계함 (tunnel())
//    tokio::io::copy_bidirectional(&mut upgraded, &mut server)
//	•	클라이언트 ↔ 프록시 ↔ tokio.rs 서버 간의 raw TCP 통신 유지됨
//. •	프록시는 내용을 해석하거나 개입하지 않음, 그냥 중계

// ✅ 실무 응용 예시
// 사내 프록시 서버 -> 인터넷 접근 통제, 로그 남기기.
// HTTPS 통과 프록시 (Man-in-the-middle) -> 보안 분석, SSL termination.
// 네트워크 디버깅 도구 -> Fiddler, Charles, mitmproxy 같은 툴.
// Kubernetes sidecar proxy -> 서비스 메시 구성 (예: Istio, Linkerd).

// 🧠 핵심 학습 포인트
// 	•	Axum + Hyper를 혼합하여 직접 http1::serve_connection()을 사용하는 구조
// 	•	CONNECT 요청 처리는 일반 HTTP 핸들링과는 달리 upgrade + 터널링 필요
// 	•	hyper 서비스와 tower service를 조합하여 유연한 요청 분기
