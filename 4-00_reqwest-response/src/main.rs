//! 서버 내부에서 HTTP 클라이언트인 reqwest를 사용하여 요청을 보내고, 그 응답을 그대로 스트리밍하는 패턴을 보여주는 중급 예제
//!
//! ```not_rust
//! cargo run -p example-reqwest-response
//! ```

use axum::{
    body::{Body, Bytes}, // Body: 응답 바디 스트림 타입, Bytes: chunk 단위 바이트 데이터
    extract::State,      // State: 공유 상태 추출
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use reqwest::Client; // HTTP 클라이언트
use std::{convert::Infallible, time::Duration};
use tokio_stream::StreamExt; // stream 편의 메서드
use tower_http::trace::TraceLayer; // 요청/응답 추적 로그
use tracing::Span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 트레이싱 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = Client::new(); // reqwest HTTP 클라이언트 생성

    let app = Router::new()
        .route("/", get(stream_reqwest_response)) // 기본 경로: /stream 의 응답을 받아서 그대로 전송
        .route("/stream", get(stream_some_data)) // 기본 경로: /stream 의 응답을 받아서 그대로 전송
        // Add some logging so we can see the streams going through
        .layer(TraceLayer::new_for_http().on_body_chunk(
            // 스트리밍 응답 본문 chunk 단위 로깅
            |chunk: &Bytes, _latency: Duration, _span: &Span| {
                tracing::debug!("streaming {} bytes", chunk.len());
            },
        ))
        .with_state(client); // 공유 상태로 reqwest 클라이언트 주입

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap(); // 서버 실행
}

// =============================
// / 요청 핸들러
// 내부적으로 /stream 으로 HTTP 요청을 보내고,
// 응답을 받은 후 클라이언트에게 그대로 전송
// =============================
async fn stream_reqwest_response(State(client): State<Client>) -> Response {
    let reqwest_response = match client.get("http://127.0.0.1:3000/stream").send().await {
        Ok(res) => res,
        Err(err) => {
            tracing::error!(%err, "request failed");
            return (StatusCode::BAD_REQUEST, Body::empty()).into_response();
        }
    };

    // 응답 헤더, 상태코드를 그대로 가져와서 재구성
    let mut response_builder = Response::builder().status(reqwest_response.status());
    *response_builder.headers_mut().unwrap() = reqwest_response.headers().clone();

    // 응답 body 는 스트리밍 방식으로 전송
    response_builder
        .body(Body::from_stream(reqwest_response.bytes_stream()))
        // This unwrap is fine because the body is empty here
        .unwrap()
}

// =============================
// /stream 요청 핸들러
// 숫자 0~4를 1초 간격으로 스트리밍 반환
// =============================
async fn stream_some_data() -> Body {
    let stream = tokio_stream::iter(0..5) // 0~4 반복
        .throttle(Duration::from_secs(1)) // 1초 간격으로
        .map(|n| n.to_string()) // 문자열로 변환
        .map(Ok::<_, Infallible>); // 결과 타입 통일
    Body::from_stream(stream)
}

// 🔍 테스트 방법
//
// # 터미널 1: 서버 실행
// cargo run -p example-reqwest-response
//
// # 터미널 2: curl 로 테스트
// curl http://127.0.0.1:3000/
