//! 이 예제는 정의되지 않은 경로(=라우트 미스)에 대해
//! 전역적으로 404 응답을 반환하는 fallback 핸들러를 설정하는 방법을 보여줍니다.
//!
//! 실행 명령어:
//!
//! ```bash
//! cargo run -p example-global-404-handler
//! ```

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// ✅ 메인 함수 – 서버 설정 및 실행

#[tokio::main]
async fn main() {
    // ✨ tracing 로그 초기화 설정
    tracing_subscriber::registry()
        .with(
            // 환경변수에서 로그 레벨 설정을 가져오고 없으면 디폴트로 현재 크레이트=debug
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 출력 포맷 추가
        .init();

    // ✨ 기본 라우터 구성
    // "/" 경로로 들어오는 GET 요청을 handler 함수로 연결
    let app = Router::new().route("/", get(handler));

    // ✨ fallback 핸들러 설정
    // 정의되지 않은 모든 경로(=404) 요청은 handler_404 가 처리하게 됨
    let app = app.fallback(handler_404);

    // ✨ 서버 실행 (127.0.0.1:3000)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// 🧩 정상 라우트 핸들러

// "/" 경로에 대한 GET 요청 핸들러
// HTML 문자열을 반환
async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

/// 🚫 404 fallback 핸들러

// 정의되지 않은 모든 경로 요청에 대해 실행되는 fallback 핸들러
// 응답 본문은 단순 텍스트지만 커스텀 응답 형식 가능
async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "ERR 404: nothing to see here..")
}
