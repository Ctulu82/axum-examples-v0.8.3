//! 이 예제는 Axum에서 HTML 폼 데이터를 수신하고 처리하는 기본 패턴을 보여줍니다.
//!
//! 실행 방법:
//!
//! ```bash
//! cargo run -p example-form
//! ```

// `Form` 추출기: 폼 데이터를 구조체로 추출
// `Html` 응답 타입
use axum::{extract::Form, response::Html, routing::get, Router};

// Serde를 이용해 폼 데이터를 구조체로 역직렬화
use serde::Deserialize;

// 로그 출력을 위한 tracing 설정
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // ✨ 로그 필터 설정 (환경변수 OR 기본 디버그 레벨)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔 출력 설정
        .init();

    // ✨ 앱 라우터 설정
    let app = app();

    // ✨ TCP 리스너 바인딩 및 Axum 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// ✨ 라우터 정의
fn app() -> Router {
    Router::new().route("/", get(show_form).post(accept_form))
}

// ✨ GET 요청: HTML 폼을 반환
async fn show_form() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <form action="/" method="post">
                    <label for="name">
                        Enter your name:
                        <input type="text" name="name">
                    </label>

                    <label>
                        Enter your email:
                        <input type="text" name="email">
                    </label>

                    <input type="submit" value="Subscribe!">
                </form>
            </body>
        </html>
        "#,
    )
}

// ✨ 폼에서 수신할 데이터 구조 정의
#[derive(Deserialize, Debug)]
#[allow(dead_code)] // 테스트 외 사용이 없더라도 경고 방지
struct Input {
    name: String,
    email: String,
}

// ✨ POST 요청 핸들러: HTML 폼 데이터 수신 및 응답 반환
async fn accept_form(Form(input): Form<Input>) -> Html<String> {
    dbg!(&input); // 터미널에 디버그 출력
    Html(format!(
        "email='{}'\nname='{}'\n",
        &input.email, &input.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use http_body_util::BodyExt; // 바디를 바이트로 수집하기 위한 유틸
    use tower::ServiceExt; // oneshot 실행에 필요한 트레잇

    // ✨ GET 요청 테스트: 폼 UI 반환
    #[tokio::test]
    async fn test_get() {
        let app = app();

        // GET "/" 요청 전송
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 응답 바디 추출 및 HTML 포함 여부 확인
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(body.contains(r#"<input type="submit" value="Subscribe!">"#));
    }

    // ✨ POST 요청 테스트: 폼 데이터 전송 및 응답 확인
    #[tokio::test]
    async fn test_post() {
        let app = app();

        // POST "/" 요청 구성
        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/")
                    .header(
                        http::header::CONTENT_TYPE,
                        mime::APPLICATION_WWW_FORM_URLENCODED.as_ref(), // "application/x-www-form-urlencoded"
                    )
                    .body(Body::from("name=foo&email=bar@axum"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 응답 바디 추출 후 문자열로 확인
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = std::str::from_utf8(&body).unwrap();

        // 기대값과 일치하는지 확인
        assert_eq!(body, "email='bar@axum'\nname='foo'\n");
    }
}
