//! Reverse Proxy 예제
//! - 4000번 포트에서 요청을 받아
//! - 3000번 포트에 실제로 프록시하여 응답을 전달합니다.
//!
//! 📌 예제 목적 요약:
//!   localhost:4000에서 수신한 모든 요청을 localhost:3000의 실제 서버로 프록시(전달) 합니다.
//!   • 외부 사용자는 4000번 포트만 사용
//!   • 내부에 존재하는 진짜 서비스는 3000번 포트에 존재
//!   • Reverse Proxy는 이 둘을 연결해주는 중간자 역할
//!
//! 🧭 동작 흐름
//! [사용자 브라우저/curl]
//!       ↓   요청: http://localhost:4000/
//!  [Reverse Proxy: 4000번 포트]
//!       ↓   요청 forwarding
//!  [실서버 (Backend): 3000번 포트]
//!       ↑   응답 반환
//!  [Reverse Proxy]
//!       ↑   응답 forwarding
//!  [사용자에게 응답]
//!

use axum::{
    body::Body,
    extract::{Request, State},
    http::uri::Uri,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use hyper::StatusCode;
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

// hyper 기반의 HTTP client 타입 정의
type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[tokio::main]
async fn main() {
    // 실서버(3000번) 먼저 띄움 (비동기 실행)
    tokio::spawn(server());

    // hyper 기반 클라이언트 생성
    let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

    // 4000번 포트에 바인딩된 리버스 프록시 서버 구성
    let app = Router::new().route("/", get(handler)).with_state(client); // 클라이언트 주입

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// 🔁 Reverse Proxy 핸들러 구현

// 4000번 포트에 들어온 요청을 3000번으로 프록시
async fn handler(State(client): State<Client>, mut req: Request) -> Result<Response, StatusCode> {
    // 요청 path 와 query 추출
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    // 새로운 URI 생성 (실서버 대상)
    let uri = format!("http://127.0.0.1:3000{}", path_query);

    // 요청 URI를 변경
    *req.uri_mut() = Uri::try_from(uri).unwrap();

    // hyper 클라이언트를 통해 요청 전달
    Ok(client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}

/// 🧭 프록시 뒤에서 실제 응답을 제공하는 `실서버` 구성 (3000번 포트)
async fn server() {
    let app = Router::new().route("/", get(|| async { "Hello, world!" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// 🧪 테스트 방법
// # 프록시 경유 요청
// curl http://localhost:4000/
// # → 프록시 서버가 받은 요청을 3000번에 전달
// # → 3000번 서버의 응답을 사용자에게 전달

// ✅ Reverse Proxy vs 일반 Proxy 비교
// 1. 주 사용 대상
//    Forward Proxy (http-proxy):
//     > 클라이언트 -> 외부 서버
//    Reverse Proxy (reverse-proxy):
//     > 클라이언트 -> `내부` 서버
//
// 2. 클라이언트 인식 대상
//    Forward Proxy (http-proxy):
//     > 외부 서버
//    Reverse Proxy (reverse-proxy):
//     > 리버스 프록시
//
// 3. 사용 예
//    Forward Proxy (http-proxy):
//     > 학교 프록시 서버, VPN
//    Reverse Proxy (reverse-proxy):
//     > Nginx, API Gateway, Load Balancer
//
// 4. TLS 처리
//    Forward Proxy (http-proxy):
//     > 프록시는 암호화 모름.
//    Reverse Proxy (reverse-proxy):
//     > 프록시는 TLS 종료 가능

// 🧠 실무 확장 아이디어
// 경로 기반 프록시: /api -> localhost:3000, /admin -> localhost:5000
// 헤더 추가: 프록시 요청에 인증 헤더 자동 삽입
// 캐싱: 프록시 응답을 캐싱하여 백엔드 부하 감소
// 로드 밸런싱: 여러 백엔드 중 하나로 요청 분산
// 보안 강화: 백엔드는 내부망만 열고, 프록시에서 인증 처리
