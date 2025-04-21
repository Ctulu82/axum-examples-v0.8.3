//! consume-body-in-extractor-or-middleware
//! > 요청 바디(Request Body)를 미들웨어 또는 추출기에서 선(先) 소비하는 방법을 설명
//! > Rust 서버 개발에서 흔히 부딪히는 “한 번 읽은 Body는 다시 읽을 수 없다” 문제를 해결하는 예제
//!

use axum::{
    body::{Body, Bytes},
    extract::{FromRequest, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use http_body_util::BodyExt; // body 수집용 확장 trait
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Router 구성
    let app = Router::new()
        .route("/", post(handler))
        .layer(middleware::from_fn(print_request_body)); // body를 미리 읽는 미들웨어 추가

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// 🔁 미들웨어에서 body 읽기

/// 요청 본문을 미리 읽고 로깅하는 미들웨어
async fn print_request_body(request: Request, next: Next) -> Result<impl IntoResponse, Response> {
    // body 를 읽고 다시 request에 붙여주는 함수
    let request = buffer_request_body(request).await?;
    Ok(next.run(request).await)
}

/// 실제 body를 read + clone + 복구하는 작업 수행
/// 커스텀 추출기에서 body 를 읽고 재사용 가능하게 구현.
/// 핵심 포인트: body는 한번 consume되면 사라지므로, Bytes로 버퍼링 후 다시 body 를 만들어야 함.
async fn buffer_request_body(request: Request) -> Result<Request, Response> {
    let (parts, body) = request.into_parts(); // request → parts + body 분리

    // body 수집 (단, streaming body가 아닌 경우만 가능)
    let bytes = body
        .collect()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response())?
        .to_bytes();

    // 원하는 작업 수행 (여기선 로깅)
    do_thing_with_request_body(bytes.clone());

    // 다시 Request 로 조립해서 반환 (body 복원)
    Ok(Request::from_parts(parts, Body::from(bytes)))
}

// 여기선 그냥 디버그 로그 출력
fn do_thing_with_request_body(bytes: Bytes) {
    tracing::debug!(body = ?bytes);
}

/// 🧲 핸들러와 추출기 구현

// 실제 라우트 핸들러: 커스텀 추출기 사용
async fn handler(BufferRequestBody(body): BufferRequestBody) {
    tracing::debug!(?body, "handler received body");
}

// 커스텀 추출기: 요청 본문을 Bytes 형태로 추출
struct BufferRequestBody(Bytes);

// body를 consume 해야 하므로 FromRequest 구현
impl<S> FromRequest<S> for BufferRequestBody
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(req, state)
            .await
            .map_err(|err| err.into_response())?;

        do_thing_with_request_body(body.clone()); // 미들웨어와 동일하게 재사용 가능

        Ok(Self(body))
    }
}

// 🧠 핵심 요점 요약
// > 요청 바디는 stream 이기 때문에 한 번만 읽을 수 있음.
// > Bytes 로 수집하고, 복제해서 Body::from() 으로 다시 만들어야 함
// > 이 방법은 미들웨어나 추출기에서 body 를 미리 파싱하거나 로깅할 때 유용함
// 주의: body 를 너무 일찍 consume 하면 이후 handler 에서 body 가 없다고 실패할 수 있음. -> 재조립 필수!

// ✅ 왜 요청 바디를 다시 읽어야 할까?
// > 스트리밍 기반 웹 서버에서 “요청 바디를 다시 읽어야 하는 이유”는 실무에서 꽤 자주 등장
// 01_ 미들웨어에서 검사: 바디 내용을 검사한 뒤, 핸들러에서도 동일한 바디를 사용하고 싶을 때.
// 02_ 로깅/감사 로그 저장: 요청본문(JSON 등)을 DB나 로그로 남기고, 이후 처리도 계속 진행.
// 03_ 서명 / 검증: HMAC, JWT 서명, 서드파티 요청(예: Webhook)의 본문 위조 검증.
// 04_ 컨텐츠 검열 또는 필터링: 악성 payload 탐지, 금칙어 차단 등의 사전 처리.
// 05_ 접근 제한 (요금제 등): 사용량이나 쿼리 복잡도 측정을 위해 먼저 검사 후 실제 핸들링.

// 📦 실무/도메인 예시
//
// 🎯 1. Webhook 인증 (Stripe, Kakao, Slack 등)
// - Webhook 요청에는 헤더에 signature가 있고
// - 본문 전체를 기반으로 HMAC-SHA256 으로 검증
// - 검증 로직은 미들웨어에서 처리하고
// - 본문 내용은 이후 handler에서 또 사용
//
// 🔐 2. 로그/감사 기록 시스템
//- 금융/의료/교육 시스템에서 요청 로그는 무조건 저장 대상
// - 요청자, 헤더, 바디(JSON) 등을 한꺼번에 감사 로그로 남김
// - 하지만 이후에도 handler는 body를 필요로 함
//
// 🧪 3. 필터링/사전 검사
// - 본문에 포함된 키워드가 금칙어인지 검사 (e.g. 욕설 필터)
// - JSON schema validation을 미들웨어에서 수행 (대역폭 낭비 방지)
//
// 🧮 4. 과금/쿼리 분석
// - AI API, DB API 등에서는 요청 본문 크기, 쿼리 복잡도 기반으로 과금
// - 바디가 예를 들어 GraphQL 쿼리라면 complexity 측정

// 🔁 Rust 기반 서버에서의 대응
// •	Axum, Actix 등은 기본적으로 body를 stream으로 처리 (hyper 기반)
// •	한 번 읽으면 다시 못 읽음
// •	해결책: collect()로 전체 body를 Bytes로 버퍼링 + 재구성 (Body::from())

// 🧭 결론
// 요청 바디를 다시 읽는 케이스는 드물지만 결정적일 때 꼭 필요하며,
// 특히 보안/감사/비즈니스 로직에서 자주 등장합니다.
//  - 보안(Webhook, HMAC) ✅ : body 재사용 필요성 매우 높음
//  - 금융/의료 시스템 로그 ✅ : body 재사용 필요성 높음
//  - 실시간 필터링/검열 ✅ : body 재사용 필요성 높음
//  - 일반 REST API 🤔 : body 재사용 필요성 거의 없음
