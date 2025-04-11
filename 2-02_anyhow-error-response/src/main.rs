//! 이 예제는 `anyhow::Error`를 Axum 응답으로 변환하여, 간결하게 에러를 처리하는 구조를 보여줍니다.
//!
//! 실행 방법:
//! ```bash
//! cargo run -p example-anyhow-error-response
//! ```

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

#[tokio::main]
async fn main() {
    // 라우터 생성
    let app = app();

    // 서버 바인딩 및 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// ✨ 요청을 처리하는 핸들러
// try_thing() 호출 → 실패 시 AppError 반환
async fn handler() -> Result<(), AppError> {
    try_thing()?; // ? 연산자 사용 가능 (From<E> for AppError 구현 덕분)
    Ok(())
}

// ✨ 실패하는 함수 (에러 발생 예시)
fn try_thing() -> Result<(), anyhow::Error> {
    // anyhow::bail! → 즉시 실패하는 Result 반환 매크로
    anyhow::bail!("it failed!")
}

// ✨ anyhow::Error 를 감싼 AppError 정의
// 이후 IntoResponse 구현을 통해 Axum 응답으로 변환
struct AppError(anyhow::Error);

// ✨ AppError → HTTP 응답 변환
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// ✨ 라우터 정의 함수
fn app() -> Router {
    Router::new().route("/", get(handler))
}

// ✨ From<E> for AppError 구현
// 덕분에 anyhow::Error 또는 그와 호환되는 에러 타입을 ? 연산자로 자동 변환 가능
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
    use axum::{body::Body, http::Request, http::StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_main_page() {
        // 라우터 생성
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/") // GET /
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // 상태코드 검증 (에러 발생했기 때문에 500)
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // 응답 바디 추출
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        // 응답 메시지 검증
        assert_eq!(html, "Something went wrong: it failed!");
    }
}
