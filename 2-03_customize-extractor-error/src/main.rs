//! 커스텀 추출기 오류 처리 예제를 실행하는 메인 엔트리 파일입니다.
//!
//! > POST 시 라우터 3개 모두 {"name":"kim"} 를 전달.
//! > 'invalid' 를 전달하면 각기 다른 에러메시지 리턴됨.
//!
//! 실행 명령어:
//! ```bash
//! cargo run -p example-customize-extractor-error
//! ```

// ✨ 각기 다른 방식의 커스텀 추출기를 구현한 모듈 임포트
mod custom_extractor;
mod derive_from_request;
mod with_rejection;

use axum::{routing::post, Router};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // ✨ tracing 로그 시스템 설정
    tracing_subscriber::registry()
        .with(
            // 환경 변수에서 로그 필터 가져오거나 기본 trace 레벨 설정
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 포맷 레이어 추가
        .init();

    // ✨ 라우터 구성: 3개의 경로를 각각의 커스텀 추출기 구현에 연결
    let app = Router::new()
        .route("/with-rejection", post(with_rejection::handler))
        .route("/custom-extractor", post(custom_extractor::handler))
        .route("/derive-from-request", post(derive_from_request::handler));

    // ✨ 서버 실행: 127.0.0.1:3000 포트
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
