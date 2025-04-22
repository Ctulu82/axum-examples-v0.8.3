//!
//! Axum에서 tower-http::TraceLayer를 활용하여 HTTP 요청 흐름을 로깅(trace) 하는 방법을 보여주는 예제
//!

use axum::{
    body::Bytes,
    extract::MatchedPath,
    http::{HeaderMap, Request},
    response::{Html, Response},
    routing::get,
    Router,
};
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // tracing 구독자 초기화 (환경 변수 기반 필터 설정 포함)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                // 기본 필터: 현재 크레이트 + tower_http + axum::rejection
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer()) // stdout 출력용 layer
        .init();

    // 라우터 구성
    let app = Router::new()
        .route("/", get(handler)) // GET / → handler 실행
        // `TraceLayer` is provided by tower-http so you have to add that as a dependency.
        // It provides good defaults but is also very customizable.
        //
        // See https://docs.rs/tower-http/0.1.1/tower_http/trace/index.html for more details.
        //
        // If you want to customize the behavior using closures here is how.
        // TraceLayer 를 통해 요청/응답 흐름을 추적
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    // Log the matched route's path (with placeholders not filled in).
                    // Use request.uri() or OriginalUri if you want the real path.
                    // 요청 수신 시 tracing span 생성
                    // MatchedPath: 예를 들어 "/users/:id" 와 같은 정적 경로
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str);

                    info_span!(
                        "http_request",                  // 스팬 이름
                        method = ?request.method(),      // HTTP 메서드: GET, POST 등
                        matched_path,                    // 추출한 라우팅 경로
                        some_other_field = tracing::field::Empty, // 나중에 record 가능
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {
                    // You can use `_span.record("some_other_field", value)` in one of these
                    // closures to attach a value to the initially empty field in the info_span
                    // created above.
                    // 요청 수신 직후 실행됨
                    // _span.record("some_other_field", value) 등으로 필드 기록 가능
                })
                .on_response(|_response: &Response, _latency: Duration, _span: &Span| {
                    // 응답 직후 실행됨
                })
                .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
                    // 바디 청크 수신 시마다 호출됨 (스트리밍 시 유용)
                })
                .on_eos(
                    |_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {
                        // 스트림 종료 시 호출됨 (eos: end of stream)
                    },
                )
                .on_failure(
                    |_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                        // 요청 처리 중 오류 발생 시 호출됨
                    },
                ),
        );

    // 서버 실행 (127.0.0.1:3000)
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// GET / 요청을 처리하는 핸들러
async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

// ✅ 핵심 구성 요소 요약
// TraceLayer: 요청/응답의 라이프사이클을 추적하는 미들웨어.
// make_span_with: 요청마다 새 tracing 스팬을 생성.
// on_request: 요청 직후 실행되는 훅.
// on_response: 응답 직후 실행되는 훅.
// on_body_chunk: 바디 청크 단위로 로그 처리(스트리밍 대응).
// on_eos: 응답 스트림 종료 시점 트리거.
// on_failure: 오류 발생 시 트리거 됨 (5xx 응답 포함).

// ⸻

// 🧪 테스트 방법
//  curl http://127.0.0.1:3000/
//  # 터미널에서 로그 출력 확인 (예: http_request 스팬)
// 	# tracing::debug!, info!, warn!, error! 수준으로 로그 필터링 가능

// ⸻

// 💡 실무 팁
// 	• TraceLayer는 거의 모든 실무 서비스에서 사용하는 기본 HTTP trace 미들웨어.
// 	• info_span!에 user_id, client_ip, endpoint 등을 .record()로 추가하면 정밀한 트래픽 분석이 가능.
// 	• Sentry, Datadog, OpenTelemetry 등과 연계하여 분산 트레이싱도 구현 가능.
