//! 요청마다 고유한 x-request-id 헤더를 생성하고, 이를 로그에 포함시켜 추적할 수 있도록 설정한 예제.
//! tower_http의 미들웨어를 이용해 각 요청에 고유한 x-request-id 헤더를 생성하고, 이를 로그 트레이싱에 활용하는 방식
//! 이는 **분산 트레이싱(distributed tracing)**의 기본 개념 중 하나이며, 마이크로서비스나 클라우드 기반 백엔드에서 매우 중요한 기능.

use axum::{
    http::{HeaderName, Request},
    response::Html,
    routing::get,
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{error, info, info_span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 사용할 헤더 이름 상수 정의
const REQUEST_ID_HEADER: &str = "x-request-id";

#[tokio::main]
async fn main() {
    // 로그 레벨 및 형식 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // 기본 로그 레벨이 없을 경우 적용할 필터 (axum의 내장 리젝션 로그까지 포함)
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 고정된 헤더 이름을 HeaderName으로 변환
    let x_request_id = HeaderName::from_static(REQUEST_ID_HEADER);

    // 미들웨어 체인 구성
    let middleware = ServiceBuilder::new()
        // 요청마다 UUID 기반 x-request-id를 생성
        .layer(SetRequestIdLayer::new(
            x_request_id.clone(),
            MakeRequestUuid,
        ))
        // 요청마다 로그 트레이싱 스팬을 생성
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                // 요청 헤더에서 request_id 추출
                let request_id = request.headers().get(REQUEST_ID_HEADER);

                match request_id {
                    // request_id가 있다면 로그 스팬에 포함
                    Some(request_id) => info_span!(
                        "http_request",
                        request_id = ?request_id,
                    ),
                    // 없다면 경고를 남기고 기본 스팬 생성
                    None => {
                        error!("could not extract request_id");
                        info_span!("http_request")
                    }
                }
            }),
        )
        // request_id 헤더를 응답에도 그대로 전달
        .layer(PropagateRequestIdLayer::new(x_request_id));

    // 라우터 구성
    let app = Router::new().route("/", get(handler)).layer(middleware);

    // 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// 기본 핸들러 (GET /)
async fn handler() -> Html<&'static str> {
    info!("Hello world!"); // 로그에 트레이싱 스팬과 함께 출력됨
    Html("<h1>Hello, World!</h1>")
}

// ✅ 핵심 개념 정리
//
// 	• SetRequestIdLayer:
//    요청마다 UUID 기반 x-request-id를 자동 생성
//
// 	• TraceLayer::make_span_with():
//    해당 request-id를 포함하는 로그 트레이싱 스팬을 생성함
//    (→ 로그를 모아 분석할 때 같은 요청 흐름을 따라가기 쉬움)
//
// 	• PropagateRequestIdLayer:
//    생성된 x-request-id를 응답에도 그대로 전달
//    (→ 클라이언트도 동일한 요청 ID로 로그 추적 가능)

// ⸻

// 🧪 테스트 방법
//
// curl -v http://localhost:3000
// # 응답 헤더에서 x-request-id 확인 가능
// # 콘솔 로그에 [request_id = "..."] 포함된 항목 출력 확인

// ⸻

// 💡 실무 활용 팁
// 	•	x-request-id는 Nginx, ALB, Cloudflare 같은 로드밸런서와도 연동될 수 있음
// 	•	이 값이 있으면 서버 측 로그와 클라이언트 트래픽을 매칭할 수 있음
// 	•	추후 Sentry, Honeycomb, Datadog 등 APM 도구에서도 활용됨
