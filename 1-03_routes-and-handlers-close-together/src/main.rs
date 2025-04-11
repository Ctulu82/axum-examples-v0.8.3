//! 이 예제는 라우트(route)와 핸들러(handler)를 **가까이 정의**하여 가독성과 유지보수성을 높이는 패턴을 보여줍니다.
//!
//! 실행 방법:
//! ```bash
//! cargo run -p example-routes-and-handlers-close-together
//! ```

use axum::{
    routing::{get, post, MethodRouter}, // GET, POST 라우터 생성 도우미
    Router,                             // 라우팅 트리 구조를 담당하는 핵심 타입
};

#[tokio::main]
async fn main() {
    // ✨ 라우터 전체를 구성합니다.
    // 라우터를 각 경로별로 나눠서 정의하고, 이를 `.merge()`로 하나의 앱에 통합합니다.
    let app = Router::new()
        .merge(root()) // "/" 라우트 등록
        .merge(get_foo()) // "/foo" GET 라우트 등록
        .merge(post_foo()); // "/foo" POST 라우트 등록

    // ✨ TCP 리스너 바인딩 (127.0.0.1:3000)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());

    // ✨ Axum 서버 실행
    axum::serve(listener, app).await.unwrap();
}

// --------------------------
// 라우트 + 핸들러 함께 정의
// --------------------------

// ✨ 1. 루트 경로 "/"
fn root() -> Router {
    // "/" 에 대한 GET 요청 핸들러
    async fn handler() -> &'static str {
        "Hello, World!"
    }

    // route 함수로 "/” 경로와 GET 핸들러를 등록한 Router 반환
    route("/", get(handler))
}

// ✨ 2. "/foo" 경로의 GET 처리
fn get_foo() -> Router {
    // "/foo" 에 대한 GET 요청 핸들러
    async fn handler() -> &'static str {
        "Hi from `GET /foo`"
    }

    route("/foo", get(handler))
}

// ✨ 3. "/foo" 경로의 POST 처리
fn post_foo() -> Router {
    // "/foo" 에 대한 POST 요청 핸들러
    async fn handler() -> &'static str {
        "Hi from `POST /foo`"
    }

    route("/foo", post(handler))
}

// --------------------------
// 헬퍼: 라우트 등록 도우미
// --------------------------

// `route(path, method_router)` 함수는
// 지정된 경로(path)에 GET/POST 등의 메서드 라우터를 연결해 새로운 Router를 반환합니다.
//
// 예: route("/foo", get(handler)) 또는 route("/foo", post(handler))
fn route(path: &str, method_router: MethodRouter<()>) -> Router {
    Router::new().route(path, method_router)
}
