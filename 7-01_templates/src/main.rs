//! Axum에서 Askama 템플릿 엔진을 사용해 서버 사이드 렌더링(SSR) 방식으로 HTML을 동적으로 생성하는 기본 구조를 보여줌.
//! > Askama: Jinja2 스타일의 Rust 템플릿 엔진.
//! ```not_rust
//! cargo run -p example-templates
//! ```
//! http://localhost:3000/greet/TaeHyun -> Hello, TaeHyun!
//!

use askama::Template; // 매크로로 HTML 템플릿과 Rust 구조체를 연결
use axum::{
    extract,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // build our application with some routes
    let app = app();

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

fn app() -> Router {
    // /greet/{name} 형태의 경로를 등록
    Router::new().route("/greet/{name}", get(greet))
}

async fn greet(extract::Path(name): extract::Path<String>) -> impl IntoResponse {
    // URL 경로의 name 값을 추출하고
    // HelloTemplate에 전달 → Hello, {{ name }}! 을 렌더링
    let template = HelloTemplate { name };
    HtmlTemplate(template)
}

/// 🎨 템플릿 구조체 선언

#[derive(Template)]
#[template(path = "hello.html")] // templates/ 디렉토리 기준
struct HelloTemplate {
    name: String,
}

/// 🧾 HtmlTemplate<T> → HTML 응답으로 변환

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_main() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/greet/Foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(html, "<h1>Hello, Foo!</h1>");
    }
}

// 🚀 실행 및 테스트 방법
//
// # 실행
// cargo run -p example-templates
//
// # 브라우저에서 확인
// http://localhost:3000/greet/Axum
//
// # 테스트
// cargo test -p example-templates
