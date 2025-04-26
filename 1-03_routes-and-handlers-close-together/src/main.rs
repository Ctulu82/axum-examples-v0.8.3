//! 이 예제는 라우트(route)와 핸들러(handler)를 **가까이 정의**하여 가독성과 유지보수성을 높이는 패턴을 보여줍니다.
//!
//! 실행 방법:
//! ```bash
//! cargo run -p example-routes-and-handlers-close-together
//! ```

use axum::{
    routing::{
        get,          // HTTP GET 요청을 처리하는 라우터 생성 함수
        post,         // HTTP POST 요청을 처리하는 라우터 생성 함수
        MethodRouter, // HTTP 메서드(GET, POST 등)를 라우트에 연결하는 타입
    },
    Router, // 전체 라우팅 트리를 구성하는 핵심 타입
};

#[tokio::main]
async fn main() {
    // ✨ 라우터 전체를 구성.
    // 각 경로별 라우터를 정의한 후, `.merge()`로 하나의 앱으로 통합합니다.
    let app = Router::new()
        .merge(root()) // "/" 라우트 등록
        .merge(get_foo()) // "/foo"에 대한 GET 라우트 등록
        .merge(post_foo()); // "/foo"에 대한 POST 라우트 등록

    // ✨ TCP 리스너 바인딩 (127.0.0.1:3000)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 처리합니다.

    // 현재 서버가 리스닝 중인 주소를 출력합니다.
    println!("listening on {}", listener.local_addr().unwrap());

    // ✨ Axum 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리합니다.
}

// MARK: - 라우트 + 핸들러 함께 정의

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

// MARK: - 헬퍼 함수: 라우트를 등록하는 도우미

/// `route(path, method_router)` 함수는
/// 지정된 경로(path)에 GET/POST 등의 메서드 라우터를 연결해 새로운 Router를 반환합니다.
///
/// 예시: `route("/foo", get(handler))` 또는 `route("/foo", post(handler))`
fn route(path: &str, method_router: MethodRouter<()>) -> Router {
    Router::new().route(path, method_router)
}
