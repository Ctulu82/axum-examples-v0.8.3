//! 이 예제는 정의되지 않은 경로(=라우트 미스)에 대해
//! 전역적으로 404 응답을 반환하는 fallback 핸들러를 설정하는 방법을 보여줍니다.
//!
//! 실행 명령어:
//!
//! ```bash
//! cargo run -p example-global-404-handler
//! ```

use axum::{
    http::StatusCode,               // HTTP 상태 코드
    response::{Html, IntoResponse}, // HTML 응답 타입과 응답 변환 트레잇
    routing::get,                   // GET 메서드 라우팅
    Router,                         // 라우터 객체
};
use tracing_subscriber::{
    layer::SubscriberExt,    // Layer 확장 기능
    util::SubscriberInitExt, // Subscriber 초기화 확장 기능
};

/// ✅ 메인 함수 – 서버 설정 및 실행

#[tokio::main]
async fn main() {
    // ✨ tracing 로그 초기화 설정
    tracing_subscriber::registry()
        .with(
            // 환경변수에서 로그 레벨 설정을 가져오고, 없으면 현재 크레이트명을 기준으로 debug 레벨 설정
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 로그 출력 포맷 적용
        .init();

    // ✨ 기본 라우터 구성
    // "/" 경로로 들어오는 GET 요청을 handler 함수로 연결
    let app = Router::new().route("/", get(handler));

    // ✨ fallback 핸들러 설정
    // 정의되지 않은 모든 경로(404 대상 요청)를 handler_404 함수로 처리
    let app = app.fallback(handler_404);

    // ✨ 서버 실행 (127.0.0.1:3000 바인딩)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 발생

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리
}

/// 🧩 정상 라우트 핸들러

// "/" 경로에 대한 GET 요청을 처리하는 핸들러
// 간단한 HTML 문자열을 반환
async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

/// 🚫 404 fallback 핸들러

// 정의되지 않은 모든 경로 요청에 대해 실행되는 fallback 핸들러
// 404 상태 코드와 에러 메시지를 반환
async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "ERR 404: nothing to see here..")
}
