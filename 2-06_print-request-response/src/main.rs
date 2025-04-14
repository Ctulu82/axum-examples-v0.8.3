//! 이 예제는 요청(Request)과 응답(Response)의 바디를 읽고 출력하는 미들웨어를 구현한 것입니다.
//!
//! - 요청 바디를 출력하고,
//! - 다음 라우터로 전달한 뒤,
//! - 응답 바디도 출력하여 최종 응답을 반환합니다.
//
//! 실행 방법:
//! ```bash
//! cargo run -p example-print-request-response
//! ```

use axum::{
    body::{Body, Bytes}, // 바디 타입
    extract::Request,    // 추출용 전체 Request
    http::StatusCode,
    middleware::{self, Next},           // 사용자 정의 미들웨어 관련
    response::{IntoResponse, Response}, // 응답 인터페이스
    routing::post,
    Router,
};
use http_body_util::BodyExt; // 바디 수집 유틸리티 (collect() 지원)
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🧭 메인 함수 – 서버 및 라우터 구성

#[tokio::main]
async fn main() {
    // ✨ tracing 로그 시스템 설정
    tracing_subscriber::registry()
        .with(
            // 환경변수 없으면 기본 디버그 설정
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 출력
        .init();

    // ✨ 라우터 정의
    let app = Router::new()
        // "/" 경로에 POST 요청 허용
        .route("/", post(|| async move { "Hello from `POST /`" }))
        // ✨ 사용자 정의 미들웨어 적용
        .layer(middleware::from_fn(print_request_response));

    // ✨ 서버 실행 (127.0.0.1:3000)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// 🧩 미들웨어 함수 정의

// 사용자 정의 미들웨어 함수
// - 요청(req)와 다음(next) 라우터를 받아 처리
// - 요청 및 응답 바디를 읽고 출력 후 다시 조립하여 넘김
async fn print_request_response(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // 요청을 분리
    let (parts, body) = req.into_parts();

    // 요청 바디를 읽고 출력
    let bytes = buffer_and_print("request", body).await?;

    // 다시 Request로 조립
    let req = Request::from_parts(parts, Body::from(bytes));

    // 다음 미들웨어/라우터 실행
    let res = next.run(req).await;

    // 응답을 분리
    let (parts, body) = res.into_parts();

    // 응답 바디를 읽고 출력
    let bytes = buffer_and_print("response", body).await?;

    // 다시 Response로 조립하여 반환
    let res = Response::from_parts(parts, Body::from(bytes));

    Ok(res)
}

/// 📦 바디 읽고 출력하는 보조 함수

// 요청 또는 응답의 바디를 읽고 출력하는 유틸 함수
async fn buffer_and_print<B>(direction: &str, body: B) -> Result<Bytes, (StatusCode, String)>
where
    B: axum::body::HttpBody<Data = Bytes>, // 바디의 데이터가 Bytes 타입
    B::Error: std::fmt::Display,           // 에러 메시지를 문자열로 출력할 수 있어야 함
{
    // ✨ 전체 바디를 Bytes 로 수집
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),

        // 실패 시 400 에러 반환
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("failed to read {direction} body: {err}"),
            ));
        }
    };

    // 문자열로 변환 가능한 경우 로그로 출력
    if let Ok(body) = std::str::from_utf8(&bytes) {
        tracing::debug!("{direction} body = {body:?}");
    }

    Ok(bytes)
}
