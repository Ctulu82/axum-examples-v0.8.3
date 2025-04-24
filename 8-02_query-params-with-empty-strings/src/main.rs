//! Axum에서 쿼리 문자열 파라미터에 빈 문자열이 올 경우 어떻게 다룰지를 보여주는 실용적인 예제.
//! 특히 Option<T> 타입에서 빈 문자열("")을 None으로 처리하고 싶을 때 유용!
//!
//! ```not_rust
//! cargo run -p example-query-params-with-empty-strings
//! ```
//! 브라우저나 Postman 에서 GET으로..
//! http://localhost:3000/?foo=&bar=bar

use axum::{extract::Query, routing::get, Router}; // Query: Axum에서 쿼리 파라미터 추출용 추출기
use serde::{de, Deserialize, Deserializer}; // serde 관련 항목은 구조체 필드의 커스텀 디시리얼라이저 작성에 필요
use std::{fmt, str::FromStr};

/// --- 🎯 메인 함수

#[tokio::main]
async fn main() {
    // 127.0.0.1:3000에서 서버 시작
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app()).await.unwrap(); // app() 함수를 통해 라우터 구성
}

/// 🧭 라우터 구성
fn app() -> Router {
    // / 경로에서 GET 요청 처리 → handler() 호출
    Router::new().route("/", get(handler))
}

/// 📦 요청 핸들러
async fn handler(Query(params): Query<Params>) -> String {
    format!("{params:?}")
}

/// --- 📐 구조체 정의 및 커스텀 디시리얼라이저 적용

/// See the tests below for which combinations of `foo` and `bar` result in
/// which deserializations.
///
/// This example only shows one possible way to do this. [`serde_with`] provides
/// another way. Use which ever method works best for you.
///
/// [`serde_with`]: https://docs.rs/serde_with/1.11.0/serde_with/rust/string_empty_as_none/index.html
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Params {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    foo: Option<i32>, // foo는 비어 있는 문자열("")이면 None으로 처리되게끔 커스텀 처리
    bar: Option<String>, // bar는 일반적인 Option<String>으로 처리 (”“는 Some(””))로 유지
}

/// 🧰 커스텀 디시리얼라이저 함수
/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    // foo=&bar=bar → foo: None, bar: Some("bar")
    // foo=1&bar=bar → foo: Some(1), bar: Some("bar")
    // foo= → 빈 문자열 → None 처리됨
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

/// ✅ 테스트 모듈
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// 다양한 쿼리 조합에 대해 결과가 어떻게 나오는지를 검증
    #[tokio::test]
    async fn test_something() {
        // send_request_get_body("foo=1&bar=bar") → "Params { foo: Some(1), bar: Some(\"bar\") }"
        assert_eq!(
            send_request_get_body("foo=1&bar=bar").await,
            r#"Params { foo: Some(1), bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=&bar=bar").await,
            r#"Params { foo: None, bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=&bar=").await,
            r#"Params { foo: None, bar: Some("") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=1").await,
            r#"Params { foo: Some(1), bar: None }"#,
        );

        assert_eq!(
            send_request_get_body("bar=bar").await,
            r#"Params { foo: None, bar: Some("bar") }"#,
        );

        assert_eq!(
            send_request_get_body("foo=").await,
            r#"Params { foo: None, bar: None }"#,
        );

        assert_eq!(
            send_request_get_body("bar=").await,
            r#"Params { foo: None, bar: Some("") }"#,
        );

        assert_eq!(
            send_request_get_body("").await,
            r#"Params { foo: None, bar: None }"#,
        );
    }

    /// test_something() 에서 호출되는 함수.
    async fn send_request_get_body(query: &str) -> String {
        let body = app()
            .oneshot(
                Request::builder()
                    .uri(format!("/?{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}

// 🔍 요약 포인트
//
// 요청 쿼리          foo 결과    bar 결과
// foo=1&bar=bar    Some(1)    Some("bar")
// foo=&bar=bar     None       Some("bar")
// foo=&bar=        None       Some("")
// foo=1            Some(1)    None
// bar=bar          None       Some("bar")
// foo=             None       None
// bar=             None       Some("")
// (빈 쿼리)          None       None
