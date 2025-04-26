//! 이 예제는 Axum에서 GET과 HEAD 요청을 어떻게 처리하는지 보여줍니다.
//!
//! ```bash
//! cargo run -p example-handle-head-request
//! ```

// 응답을 반환할 때 사용하는 타입들
use axum::response::{
    IntoResponse, // IntoResponse: 사용자 정의 타입이나 결과를 HTTP 응답(Response)로 변환할 수 있도록 해주는 트레잇
    Response,     // Response: HTTP 응답을 나타내는 기본 타입 (상태 코드, 헤더, 바디를 포함)
};

// http 요청/응답 타입을 제공하는 모듈과 라우팅 도우미
use axum::{
    http,         // http: HTTP 관련 타입 제공
    routing::get, // get: HTTP GET 요청을 위한 라우터 헬퍼 함수
    Router,       // Router: 라우팅 테이블을 정의하는 핵심 구조체
};

// ✨ 1. 앱 라우터 생성 함수
fn app() -> Router {
    // Axum에서는 GET 핸들러가 자동으로 HEAD 요청도 처리합니다.
    Router::new().route(
        "/get-head",           // 경로("/get-head")에 대해
        get(get_head_handler), // GET 요청이 들어오면 get_head_handler 함수를 호출하도록 설정합니다.
    )
}

// ✨ 2. 메인 함수: 서버 실행
#[tokio::main]
async fn main() {
    // 127.0.0.1:3000 포트에 TCP 리스너 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 처리합니다.

    // 현재 서버가 리스닝 중인 주소를 출력합니다.
    println!("listening on {}", listener.local_addr().unwrap());

    // app() 라우터를 Axum 서버에 넘겨서 실행합니다.
    axum::serve(listener, app())
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리합니다.
}

// ✨ 3. GET 핸들러 함수
// GET 요청뿐 아니라 HEAD 요청도 이 함수로 전달됩니다.
// 단, HEAD 요청의 경우 Axum은 응답 바디를 자동으로 제거합니다.
// Method를 파라미터로 받아 GET과 HEAD를 구분 처리합니다.
async fn get_head_handler(method: http::Method) -> Response {
    // HEAD 요청인 경우: 바디 없이 헤더만 포함한 응답을 반환합니다.
    if method == http::Method::HEAD {
        return ([("x-some-header", "header from HEAD")]).into_response();
    }

    // GET 요청인 경우: 연산 수행 후 바디와 헤더를 포함한 응답을 반환합니다.
    do_some_computing_task();

    // 헤더와 바디가 함께 포함된 응답을 반환합니다.
    ([("x-some-header", "header from GET")], "body from GET").into_response()
}

// ✨ 4. GET 요청에서 사용될 연산 함수 (지금은 빈 함수)
fn do_some_computing_task() {
    // 실제 계산, DB 조회, 파일 읽기 등 비용이 드는 작업이 들어갈 수 있습니다.
    // (현재 예제에서는 빈 함수입니다.)
}

// MARK: - Test Section

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
