//! 이 예제는 Axum의 가장 기본적인 "Hello, World!" 웹 서버를 구현한 코드입니다.
//!
//! 실행 방법:
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

// Axum 프레임워크에서 필요한 모듈들을 임포트합니다.
use axum::{
    response::Html, // Html: HTML 응답을 생성하는 타입
    routing::get,   // get: HTTP GET 요청을 위한 라우터 헬퍼 함수
    Router,         // Router: 라우팅 테이블을 정의하는 핵심 구조체
};

// 메인 비동기 함수입니다.
// Tokio 런타임 기반으로 실행됩니다.
#[tokio::main]
async fn main() {
    // 1. 라우터 정의
    // 새로운 Router를 생성합니다.
    let app = Router::new().route(
        "/",          // 루트 경로("/")에 대해
        get(handler), // GET 요청이 들어오면 handler 함수를 호출하도록 설정합니다.
    );

    // 2. 서버 바인딩
    // 127.0.0.1:3000 주소에 TCP 리스너를 바인딩합니다.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 처리합니다.

    // 3. 현재 서버가 리스닝 중인 주소를 출력합니다.
    println!("listening on {}", listener.local_addr().unwrap());

    // 4. Axum 서버 실행
    // 리스너(listener)와 라우터(app)를 넘겨서 HTTP 서버를 실행합니다.
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리합니다.
}

/// 요청이 들어오면 호출되는 핸들러 함수입니다.
/// - Html<&'static str> 타입은 정적인 HTML 문자열을 감쌉니다.
async fn handler() -> Html<&'static str> {
    // HTML 콘텐츠를 반환합니다.
    Html("<h1>Hello, World!</h1>")
}
