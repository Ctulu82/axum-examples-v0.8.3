//! 이 예제는 `axum::extract::FromRequest`를 derive 하여
//! 기존 추출기(`axum::Json`)를 감싸고,
//! 에러 발생 시 **커스텀 에러 타입(`ApiError`)로 변환**하는 방법을 보여줍니다.
//!
//! ✅ 장점: 간단하고 깔끔하게 사용자 정의 추출기를 생성할 수 있음
//! ❎ 단점: rejection 타입마다 반복적인 보일러플레이트 코드 필요
//!
//! 관련 문서:
//! - thiserror: https://crates.io/crates/thiserror
//! - known limitations (제약 사항): https://docs.rs/axum-macros/latest/axum_macros/derive.FromRequest.html#known-limitations

use axum::{
    extract::rejection::JsonRejection, // JSON 파싱 실패 시 발생하는 표준 Rejection 타입
    extract::FromRequest,              // 커스텀 추출기를 만들기 위한 트레잇
    http::StatusCode,                  // HTTP 상태 코드 정의용
    response::IntoResponse,            // 응답 변환을 위한 트레잇
};
use serde::Serialize;
use serde_json::{
    json,  // JSON 객체를 쉽게 생성할 수 있는 매크로
    Value, // 동적 JSON 값 타입 (구조를 모르는 JSON 데이터 처리용)
};

// ✨ 요청 핸들러 함수
// - 우리가 만든 커스텀 Json<T> 추출기를 사용 (내부적으로 axum::Json<Value> 사용)
pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value)) // 받은 JSON 값을 디버깅 출력하고 그대로 반환
}

/// 🧩 커스텀 추출기 및 응답 변환 구현

// ✨ 커스텀 Json<T> 추출기 정의
//
// #[from_request(via = axum::Json, rejection = ApiError)]:
// - 내부적으로 `axum::Json`을 통해 데이터를 추출
// - 실패 시 `ApiError` 타입으로 변환
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Json<T>(T);

// ✨ Json<T> → 응답 변환 구현
// - 핸들러에서 Json<T>를 반환할 때 자동으로 JSON 응답으로 변환
impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        let Self(value) = self;
        axum::Json(value).into_response()
    }
}

/// 🚨 커스텀 에러 타입 정의

// ✨ ApiError: 요청 파싱 실패 시 사용자 정의 에러 타입
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode, // HTTP 상태 코드
    message: String,    // 에러 메시지
}

// ✨ JsonRejection → ApiError 변환
// - 표준 Json 파싱 에러를 커스텀 ApiError 타입으로 변환
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self {
            status: rejection.status(),
            message: rejection.body_text(),
        }
    }
}

// ✨ ApiError → HTTP 응답 변환
// - ApiError를 JSON 형식의 응답으로 변환
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let payload = json!({
            "message": self.message,
            "origin": "derive_from_request", // 에러 발생 위치 표시
        });

        (self.status, axum::Json(payload)).into_response()
    }
}
