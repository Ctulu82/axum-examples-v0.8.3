//! 커스텀 추출기 오류 처리 예제를 실행하는 메인 엔트리 파일입니다.
//!
//! - POST 요청 시 라우터 3개에 모두 {"name":"kim"} 형식의 JSON 데이터를 전달해보세요.
//! - 잘못된 데이터를 (예: 'invalid') 전달하면 각 방식별로 다른 에러 메시지가 반환됩니다.
//!

// --- 각기 다른 방식으로 커스텀 추출기를 구현한 모듈들 임포트 ---
mod extractors;

use extractors::custom_extractor;
use extractors::derive_from_request;
use extractors::with_rejection;

use axum::{
    routing::post, // POST 메서드용 라우터 빌더
    Router,        // 여러 라우트를 하나로 묶는 라우터 객체
};
use tracing_subscriber::{
    layer::SubscriberExt,    // 트레이싱 구독자 설정 확장 메서드
    util::SubscriberInitExt, // 트레이싱 구독자 초기화 도우미
};

#[tokio::main]
async fn main() {
    // ✨ tracing 기반 로그 시스템 초기화
    tracing_subscriber::registry()
        .with(
            // 환경 변수에서 로그 레벨을 가져오거나, 기본 trace 레벨 설정
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer()) // 로그 출력 포맷 지정
        .init();

    // ✨ 라우터 생성
    // - 각 경로마다 서로 다른 커스텀 추출기 핸들러를 연결
    let app = Router::new()
        .route("/with-rejection", post(with_rejection::handler)) // WithRejection 방식
        .route("/custom-extractor", post(custom_extractor::handler)) // 수동 구현 방식
        .route("/derive-from-request", post(derive_from_request::handler)); // derive 매크로 방식

    // ✨ 서버 소켓 바인딩 및 실행 (127.0.0.1:3000)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 발생

    tracing::debug!(
        "서버가 {} 에서 요청을 기다립니다.",
        listener.local_addr().unwrap()
    );

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리
}
