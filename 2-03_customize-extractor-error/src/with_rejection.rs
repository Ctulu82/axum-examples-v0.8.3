//! `axum_extra::extract::WithRejection`을 사용하여
//! 기존 추출기(extractor)의 실패를 커스텀 에러로 변환하는 방법을 보여줍니다.
//!
//! 장점: 기존 추출기를 그대로 감싸서 쓰므로 학습 곡선이 낮고 구현이 간단합니다.
//! 단점: 타입이 길어져 읽기 어려울 수 있고, type alias를 디스트럭처링 할 수 없습니다.

use axum::{extract::rejection::JsonRejection, response::IntoResponse, Json};
use axum_extra::extract::WithRejection;
use serde_json::{json, Value};
use thiserror::Error;

// ✨ 요청 핸들러 함수
pub async fn handler(
    // WithRejection: Json<Value> 추출기를 감싸는 래퍼입니다.
    // → Json 파싱 실패 시 JsonRejection → ApiError 로 변환됨
    //
    // 두 번째 파라미터(_)는 WithRejection이 내부적으로 필요하지만
    // 실사용에서는 무시해도 됩니다.
    WithRejection(Json(value), _): WithRejection<Json<Value>, ApiError>,
) -> impl IntoResponse {
    // 받은 JSON 값을 로그로 출력 후 다시 그대로 응답
    Json(dbg!(value))
}

// ✨ 에러 타입 정의
// `thiserror` 매크로를 사용해 `From<JsonRejection>` 구현을 자동 생성합니다.
#[derive(Debug, Error)]
pub enum ApiError {
    // `#[from]` 덕분에 JsonRejection → ApiError 변환이 자동으로 가능해집니다.
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
}

// ✨ ApiError → HTTP 응답 변환 구현
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        // 에러 종류에 따라 상태 코드 및 메시지 분기
        let (status, message) = match self {
            ApiError::JsonExtractorRejection(json_rejection) => {
                (json_rejection.status(), json_rejection.body_text())
            }
        };

        // JSON 형식의 에러 응답 생성
        let payload = json!({
            "message": message,
            "origin": "with_rejection" // 현재 파일의 출처 정보
        });

        // 상태 코드 + JSON 응답 반환
        (status, Json(payload)).into_response()
    }
}
