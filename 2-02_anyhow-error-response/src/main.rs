//! 이 예제는 `anyhow::Error`를 Axum 응답으로 변환하여, 간결하게 에러를 처리하는 구조를 보여줍니다.
//!
//! 실행 방법:
//! ```bash
//! cargo run -p example-anyhow-error-response
//! ```

// -- ✨ 외부 라이브러리 임포트
use axum::{
    http::StatusCode,                   // HTTP 상태 코드(200, 404, 500 등) 정의
    response::{IntoResponse, Response}, // 핸들러 반환 타입을 HTTP 응답으로 변환하는 트레이트와 실제 응답 타입
    routing::get,                       // GET 메서드용 라우터 빌더
    Router,                             // 라우트들을 모아서 앱을 구성하는 메인 객체
};

// -- ✨ 메인 함수

#[tokio::main]
async fn main() {
    // 라우터 생성
    let app = app();

    // ✨ 127.0.0.1:3000 포트에서 TCP 소켓 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 발생

    println!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리
}

// ✨ 요청을 처리하는 핸들러
async fn handler() -> Result<(), AppError> {
    try_thing()?; // try_thing 호출 후 실패하면 ? 연산자로 에러 전파
    Ok(())
}

// ✨ 실패하는 함수 (에러 발생 예시)
fn try_thing() -> Result<(), anyhow::Error> {
    // anyhow::bail! 매크로로 즉시 에러 반환
    anyhow::bail!("it failed!")
}

// ✨ anyhow::Error를 감싼 AppError 타입 정의
struct AppError(anyhow::Error);

// ✨ AppError를 HTTP 응답으로 변환
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR, // 500 Internal Server Error
            format!("Something went wrong: {}", self.0), // 에러 메시지를 포함한 응답 본문
        )
            .into_response()
    }
}

// ✨ 라우터 정의 함수
fn app() -> Router {
    Router::new().route("/", get(handler)) // GET / 요청을 handler로 연결
}

// ✨ From 트레이트를 구현하여 다양한 에러를 AppError로 변환 가능하게 함
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

/// 🧪 테스트 코드

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, http::StatusCode}; // 테스트용 요청/응답 타입
    use http_body_util::BodyExt; // HTTP 응답 바디 유틸리티
    use tower::ServiceExt; // oneshot(단일 요청 처리) 확장 메서드

    #[tokio::test]
    async fn test_main_page() {
        // ✨ 테스트용 라우터 생성
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/") // GET / 요청 생성
                    .body(Body::empty()) // 빈 요청 본문
                    .unwrap(),
            )
            .await
            .unwrap();

        // ✨ 상태 코드 검증
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // ✨ 응답 바디 읽기
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        // ✨ 응답 메시지 검증
        assert_eq!(html, "Something went wrong: it failed!");
    }
}
