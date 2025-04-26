//! 이 예제는 Axum에서 HTML 폼 데이터를 수신하고 처리하는 기본 패턴을 보여줍니다.
//!
//! 실행 방법:
//!
//! ```bash
//! cargo run -p example-form
//! ```

// Axum 관련 주요 모듈 임포트
use axum::{
    extract::Form,  // Form: 폼 데이터를 추출해 구조체로 매핑하는 추출기
    response::Html, // Html: HTML 콘텐츠를 반환하는 응답 타입
    routing::get,   // get: HTTP GET 요청용 라우터 생성 함수
    Router,         // Router: 전체 라우팅 트리 구조를 담당하는 타입
};

// Serde를 이용해 폼 데이터를 구조체로 역직렬화
use serde::Deserialize;

// 트레이싱(로깅) 설정을 위한 서브스크라이버 관련 모듈
use tracing_subscriber::{
    layer::SubscriberExt,    // 레이어 추가 기능
    util::SubscriberInitExt, // 서브스크라이버 초기화 기능
};

#[tokio::main]
async fn main() {
    // ✨ 로그 필터 설정 (환경변수 OR 기본 디버그 레벨)
    tracing_subscriber::registry()
        .with(
            // 환경 변수(RUST_LOG)에서 로그 레벨을 가져오고,
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug",               // 실패하면 기본으로 "debug" 레벨을 사용.
                    env!("CARGO_CRATE_NAME")  // 현재 패키지 이름 기준.
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer()) // 로그를 콘솔에 출력하는 포맷 레이어 추가
        .init(); // 초기화 실행

    // ✨ 앱 라우터 설정
    let app = app();

    // ✨ TCP 리스너 바인딩 및 Axum 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // ✨ Axum 서버를 실행하여 요청 수신
    axum::serve(listener, app).await.unwrap();
}

// ✨ 라우터 구성 함수
fn app() -> Router {
    Router::new().route(
        "/",                              // "/" 경로에 대해
        get(show_form).post(accept_form), // GET과 POST 요청을 각각 처리합니다.
    )
}

// ✨ GET 요청 처리 핸들러
// 폼을 보여주는 HTML 페이지를 반환합니다.
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
#[allow(dead_code)] // (예제에서는 사용하지 않는 필드가 있어도 경고를 무시)
struct Input {
    name: String,
    email: String,
}

// ✨ POST 요청 처리 핸들러
// 폼 데이터를 받아서 간단한 텍스트 응답을 생성합니다.
async fn accept_form(Form(input): Form<Input>) -> Html<String> {
    dbg!(&input); // 터미널에 폼 데이터 디버그 출력

    Html(format!(
        "email='{}'\nname='{}'\n",
        &input.email, &input.name
    ))
}

// MARK: - ✨ 테스트 모듈

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
                    .body(Body::from("name=foo&email=bar@axum")) // 폼 데이터 전송
                    .unwrap(),
            )
            .await
            .unwrap();

        // 응답 상태 코드가 200 OK 인지 확인
        assert_eq!(response.status(), StatusCode::OK);

        // 응답 바디 추출 후 문자열로 확인
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = std::str::from_utf8(&body).unwrap();

        // 폼 입력값이 올바르게 반영되었는지 검증
        assert_eq!(body, "email='bar@axum'\nname='foo'\n");
    }
}
