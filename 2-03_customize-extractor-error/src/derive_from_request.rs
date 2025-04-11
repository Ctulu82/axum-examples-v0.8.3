//! 이 예제는 `axum::extract::FromRequest`를 derive 하여
//! 기존 추출기(`axum::Json`)를 감싸고,
//! 에러가 발생할 경우 **커스텀 에러 타입(`ApiError`)로 변환**하는 방법을 보여줍니다.
//!
//! + 장점: 간단하게 사용자 정의 추출기를 만들 수 있음
//! - 단점: rejection 타입마다 반복적인 보일러플레이트가 필요
//!
//! 관련 문서:
//! - thiserror: https://crates.io/crates/thiserror
//! - known limitations: https://docs.rs/axum-macros/latest/axum_macros/derive.FromRequest.html#known-limitations

use axum::{
    extract::rejection::JsonRejection,
    extract::FromRequest, // 커스텀 추출기를 만들기 위한 트레잇
    http::StatusCode,
    response::IntoResponse, // 응답 변환용 트레잇
};
use serde::Serialize;
use serde_json::{json, Value};

// ✨ 요청 핸들러 함수
// Json<Value> 는 우리가 만든 Json<T> 추출기이며, 내부적으로 axum::Json<Value>를 사용합니다.
pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value)) // 받은 값 확인 후 JSON 응답
}

/// 🧩 커스텀 추출기 정의 및 응답 변환

// ✨ 커스텀 추출기 정의
//
// #[from_request(via = ...)]:
// → axum::Json<T> 를 내부적으로 사용하되,
// → 실패 시 ApiError 로 변환되도록 설정
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(T);

// ✨ Json<T> → 응답 변환 구현
impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        let Self(value) = self;
        axum::Json(value).into_response()
    }
}

/// 🚨 커스텀 에러 타입 정의

// ✨ 커스텀 에러 타입 정의
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

// ✨ axum::Json 추출 실패 → ApiError 로 변환
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self {
            status: rejection.status(),
            message: rejection.body_text(),
        }
    }
}

// ✨ ApiError → 응답 변환 구현
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.message,
            "origin": "derive_from_request" // 예제 출처명 표시
        });

        (self.status, axum::Json(payload)).into_response()
    }
}
