//! 이 예제는 Axum에서 GET과 HEAD 요청을 어떻게 처리하는지 보여줍니다.
//!
//! ```bash
//! cargo run -p example-handle-head-request
//! ```

// 응답을 반환할 때 사용하는 타입들
use axum::response::{IntoResponse, Response};

// http 요청/응답 타입을 제공하는 모듈과 라우팅 도우미
use axum::{http, routing::get, Router};

// ✨ 1. 앱 라우터 생성 함수
fn app() -> Router {
    // "/get-head" 경로에 대해 GET 요청을 받을 때 get_head_handler 핸들러를 실행합니다.
    // Axum에서는 GET 핸들러가 자동으로 HEAD 요청도 수신합니다.
    Router::new().route("/get-head", get(get_head_handler))
}

// ✨ 2. 메인 함수: 서버 실행
#[tokio::main]
async fn main() {
    // 127.0.0.1:3000 포트에 TCP 리스너 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    // app() 라우터를 Axum 서버에 넘겨서 실행
    axum::serve(listener, app()).await.unwrap();
}

// ✨ 3. GET 핸들러 함수
// GET 요청뿐 아니라 HEAD 요청도 이 함수로 들어오게 됩니다.
// 단, HEAD 요청의 경우 Axum은 응답 바디를 자동으로 제거합니다.
// Method를 파라미터로 받아 GET과 HEAD를 구분 처리합니다.
async fn get_head_handler(method: http::Method) -> Response {
    // HEAD 요청인 경우: 바디는 반환하지 않고, 헤더만 포함한 응답을 반환
    if method == http::Method::HEAD {
        return ([("x-some-header", "header from HEAD")]).into_response();
    }

    // GET 요청인 경우: 연산 수행 후 바디와 헤더를 포함한 응답을 반환
    do_some_computing_task();

    // 헤더와 바디가 함께 포함된 응답을 반환
    ([("x-some-header", "header from GET")], "body from GET").into_response()
}

// ✨ 4. GET 요청에서 사용될 연산 함수 (지금은 빈 함수)
fn do_some_computing_task() {
    // 실제 계산, DB 조회, 파일 읽기 등 비용 있는 작업이 들어갈 수 있음
    // 예제에서는 빈 함수
}

// ✨ 5. 테스트 코드 모듈
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt; // 바디를 바이트로 수집하기 위한 유틸
    use tower::ServiceExt; // oneshot 실행을 위한 trait

    // ✅ GET 요청 테스트
    #[tokio::test]
    async fn test_get() {
        let app = app();

        // GET 요청 생성
        let response = app
            .oneshot(Request::get("/get-head").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // 상태 코드 확인
        assert_eq!(response.status(), StatusCode::OK);

        // 헤더 확인
        assert_eq!(response.headers()["x-some-header"], "header from GET");

        // 바디 수집 후 바이트 비교
        let body = response.collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"body from GET");
    }

    // ✅ HEAD 요청 테스트
    #[tokio::test]
    async fn test_implicit_head() {
        let app = app();

        // HEAD 요청 생성
        let response = app
            .oneshot(Request::head("/get-head").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // 상태 코드 확인
        assert_eq!(response.status(), StatusCode::OK);

        // HEAD 응답의 헤더가 HEAD 용임을 확인
        assert_eq!(response.headers()["x-some-header"], "header from HEAD");

        // 바디가 비어 있는지 확인 (Axum이 자동으로 제거함)
        let body = response.collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
    }
}
