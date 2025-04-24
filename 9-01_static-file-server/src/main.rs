//! **정적 파일(Static Files)**을 여러 방식으로 서비스하는 다양한 패턴을 보여주는 예제
//!
//! 📦 전체 예제 요약
//!  •	assets/index.html → "Hi from index.html"
//!	 •	assets/script.js → console.log("Hello, World!");
//!	 •	7개의 포트(3001~3006, 3307)에서 각각 다른 라우팅 전략으로 정적 파일 서빙 테스트
//!
//! ```not_rust
//! cargo run -p example-static-file-server
//! ```

use axum::{
    extract::Request, handler::HandlerWithoutStateExt, http::StatusCode, routing::get, Router,
};
use std::net::SocketAddr;
use tower::ServiceExt;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 동시에 여러 포트에서 서로 다른 정적 파일 서비스 예제를 실행.
    tokio::join!(
        serve(using_serve_dir(), 3001),
        serve(using_serve_dir_with_assets_fallback(), 3002),
        serve(using_serve_dir_only_from_root_via_fallback(), 3003),
        serve(using_serve_dir_with_handler_as_service(), 3004),
        serve(two_serve_dirs(), 3005),
        serve(calling_serve_dir_from_a_handler(), 3006),
        serve(using_serve_file_from_a_route(), 3307),
    );
}

// --- 📂 개별 라우터 구성 설명

// 기본적인 ServeDir 사용을 보여주는 함수 (포트: 3001)
// /assets/index.html, /assets/script.js 경로로 접근
fn using_serve_dir() -> Router {
    // serve the file in the "assets" directory under `/assets`
    Router::new().nest_service("/assets", ServeDir::new("assets"))
}

// /assets 내부 요청 실패 시 fallback 파일 지정 테스트 함수 (포트: 3002)
fn using_serve_dir_with_assets_fallback() -> Router {
    // `ServeDir` allows setting a fallback if an asset is not found
    // so with this `GET /assets/doesnt-exist.jpg` will return `index.html`
    // rather than a 404
    // • /assets/없는파일.jpg 요청 시 404 대신 index.html 반환
    // • SPA(Single Page Application)에 적합
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .nest_service("/assets", serve_dir.clone())
        .fallback_service(serve_dir)
}

// /assets 없이 루트로 직접 정적 파일 서빙을 테스트하는 함수 (포트: 3003)
// /index.html, /script.js 등 루트에서 바로 제공
fn using_serve_dir_only_from_root_via_fallback() -> Router {
    // you can also serve the assets directly from the root (not nested under `/assets`)
    // by only setting a `ServeDir` as the fallback
    let serve_dir = ServeDir::new("assets").not_found_service(ServeFile::new("assets/index.html"));

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

// 404 발생 시 커스텀 핸들러로 "Not found" 텍스트 반환하는 함수 (포트: 3004)
// fallback으로 동작
fn using_serve_dir_with_handler_as_service() -> Router {
    async fn handle_404() -> (StatusCode, &'static str) {
        (StatusCode::NOT_FOUND, "Not found")
    }

    // you can convert handler function to service
    let service = handle_404.into_service();

    let serve_dir = ServeDir::new("assets").not_found_service(service);

    Router::new()
        .route("/foo", get(|| async { "Hi from /foo" }))
        .fallback_service(serve_dir)
}

// 멀티 정적 디렉토리 설정 예시 함수 (포트: 3005)
// /assets/index.html, /dist/anything.ext 모두 서빙 가능
fn two_serve_dirs() -> Router {
    // you can also have two `ServeDir`s nested at different paths
    let serve_dir_from_assets = ServeDir::new("assets");
    let serve_dir_from_dist = ServeDir::new("dist");

    Router::new()
        .nest_service("/assets", serve_dir_from_assets)
        .nest_service("/dist", serve_dir_from_dist)
}

// 핸들러 내부에서 직접 ServeDir을 호출 (포트: 3006)
// • 필요에 따라 조건문 등 논리 추가 가능
// • 더 유연한 컨트롤이 필요한 경우 유용
#[allow(clippy::let_and_return)]
fn calling_serve_dir_from_a_handler() -> Router {
    // via `tower::Service::call`, or more conveniently `tower::ServiceExt::oneshot` you can
    // call `ServeDir` yourself from a handler
    Router::new().nest_service(
        "/foo",
        get(|request: Request| async {
            let service = ServeDir::new("assets");
            let result = service.oneshot(request).await;
            result
        }),
    )
}

// 라우팅 단일 파일을 정해진 경로로 서빙 (포트: 3307)
// /foo 요청 시 항상 index.html 하나만 반환
fn using_serve_file_from_a_route() -> Router {
    Router::new().route_service("/foo", ServeFile::new("assets/index.html"))
}

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}

// 🔍 테스트 방법
//
// 유의사항!: 반드시 터미널에서 서버를 실행할 것!!
// cargo run -p example-static-file-server
//
// # 기본 정적 자산 보기
// curl http://127.0.0.1:3001/assets/index.html

// # 루트 fallback 확인 (SPA 용도)
// curl http://127.0.0.1:3002/assets/없는파일.jpg

// # 루트에서 직접 접근
// curl http://127.0.0.1:3003/index.html

// # 커스텀 404 메시지 확인
// curl http://127.0.0.1:3004/없는파일

// # route_service 사용
// curl http://127.0.0.1:3307/foo
