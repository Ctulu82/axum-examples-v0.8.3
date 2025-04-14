//! Content-Type 값에 따라 JSON 또는 Form 데이터를 파싱하는 추출기를 만드는 예제입니다.
//! - application/json → serde_json 기반 파싱
//! - application/x-www-form-urlencoded → URL-encoded form 파싱

/// 📦 의존 라이브러리와 타입 정의
use axum::{
    extract::{FromRequest, Request}, // 커스텀 추출기 구현에 필요한 트레잇
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Form,
    Json,
    RequestExt,
    Router, // Form, Json 기본 추출기
};
use serde::{Deserialize, Serialize}; // 역직렬화용
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🚀 서버 실행 & 라우터 구성

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            // 로그 레벨을 환경 변수에서 가져오거나 디폴트로 설정
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 로그 포맷
        .init();

    // 라우터 구성: POST / 요청은 handler 함수로 연결
    let app = Router::new().route("/", post(handler));

    // 서버 바인딩 및 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// 📨 수신 데이터 구조체 정의

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
    foo: String,
}

/// 🧾 요청 처리 핸들러

async fn handler(JsonOrForm(payload): JsonOrForm<Payload>) {
    dbg!(payload); // 요청 본문을 디버그 출력
}

/// 🧠 핵심: 커스텀 추출기 JsonOrForm<T> 구현

// Content-Type 에 따라 Json<T> 또는 Form<T> 중 적절히 추출
struct JsonOrForm<T>(T);

/// 💡 FromRequest 수동 구현

impl<S, T> FromRequest<S> for JsonOrForm<T>
where
    S: Send + Sync,
    Json<T>: FromRequest<()>, // Json 추출 가능
    Form<T>: FromRequest<()>, // Form 추출 가능
    T: 'static,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // Content-Type 헤더 추출
        let content_type_header = req.headers().get(CONTENT_TYPE);
        let content_type = content_type_header.and_then(|value| value.to_str().ok());

        // Content-Type 이 존재한다면...
        if let Some(content_type) = content_type {
            // application/json 인 경우 Json<T> 추출 시도
            if content_type.starts_with("application/json") {
                let Json(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }

            // application/x-www-form-urlencoded 인 경우 Form<T> 추출 시도
            if content_type.starts_with("application/x-www-form-urlencoded") {
                let Form(payload) = req.extract().await.map_err(IntoResponse::into_response)?;
                return Ok(Self(payload));
            }
        }

        // 지원되지 않는 Content-Type 이면 415 응답
        Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response())
    }
}

// ✅ 테스트 방법

// 1. JSON 요청
// curl -X POST http://localhost:3000 \
//      -H "Content-Type: application/json" \
//      -d '{"foo": "hello-json"}'
// ➡ 서버 콘솔:
// [src/main.rs:handler] payload = Payload { foo: "hello-json" }
//
// 2. Form 요청
// curl -X POST http://localhost:3000 \
//      -H "Content-Type: application/x-www-form-urlencoded" \
//      -d 'foo=hello-form'
// ➡ 서버 콘솔:
// [src/main.rs:handler] payload = Payload { foo: "hello-form" }
