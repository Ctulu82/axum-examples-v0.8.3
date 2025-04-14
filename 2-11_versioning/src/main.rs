//! URL 경로에 포함된 "버전 정보"를 기반으로 처리 로직을 분기하는 예제.
//! > /v1/foo, /v2/foo 등에서 "v1", "v2"를 추출하고,
//! > 이를 Enum으로 변환해 핸들러에서 활용하는 방식.
//! API 버전 관리 시 매우 실용적인 패턴이며, 실무에서도 흔히 쓰이는 구조.

use axum::{
    extract::{FromRequestParts, Path}, // 커스텀 추출기 + 경로 변수 추출
    http::{request::Parts, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    RequestPartsExt,
    Router,
};
use std::collections::HashMap;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🧭 main 함수

#[tokio::main]
async fn main() {
    // tracing 로그 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 라우터 빌드 및 실행
    let app = app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// 🧱 라우터 구성

fn app() -> Router {
    // /{version}/foo 경로에 대응
    Router::new().route("/{version}/foo", get(handler))
    // 여기서 {version}은 동적 경로 파라미터이며, 이후에 Version 타입으로 변환됨.
}

/// 📩 핸들러

async fn handler(version: Version) -> Html<String> {
    Html(format!("received request with version {version:?}"))
    // version은 자동으로 Version enum으로 파싱된 결과.
}

/// 🧠 핵심 로직: 커스텀 추출기 구현 (Version enum)

#[derive(Debug)]
enum Version {
    V1,
    V2,
    V3,
}

impl<S> FromRequestParts<S> for Version
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 경로 변수 전체를 HashMap 으로 파싱
        let params: Path<HashMap<String, String>> =
            parts.extract().await.map_err(IntoResponse::into_response)?;

        // "version" 파라미터 가져오기
        let version = params
            .get("version")
            .ok_or_else(|| (StatusCode::NOT_FOUND, "version param missing").into_response())?;

        // 문자열을 enum 으로 매핑
        match version.as_str() {
            "v1" => Ok(Version::V1),
            "v2" => Ok(Version::V2),
            "v3" => Ok(Version::V3),
            _ => Err((StatusCode::NOT_FOUND, "unknown version").into_response()),
        }
    }
}

/// 🧪 테스트 코드

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, http::StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // ✅ v1 요청 성공
    #[tokio::test]
    async fn test_v1() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v1/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(html, "received request with version V1");
    }

    // v4 요청 실패 (없는 버전)
    #[tokio::test]
    async fn test_v4() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/v4/foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(html, "unknown version");
    }
}
