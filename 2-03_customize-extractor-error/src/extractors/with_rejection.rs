//! `axum_extra::extract::WithRejection`을 사용하여
//! 기존 추출기(extractor)의 실패를 커스텀 에러로 변환하는 방법을 보여줍니다.
//!
//! ✅ 장점: 기존 추출기를 그대로 감싸 사용하므로 학습 난이도가 낮고 구현이 간단함
//! ❎ 단점: 타입이 길어지고, type alias(별칭)로 디스트럭처링이 불가능
//!

use axum::{
    extract::rejection::JsonRejection, // JSON 추출 실패 시 발생하는 기본 에러 타입
    response::IntoResponse,            // 응답 변환용 트레잇
    Json,                              // 기본 Json 추출기
};
use axum_extra::extract::WithRejection; // 추출 실패를 커스텀 에러로 감싸주는 도구
use serde_json::{
    json,  // JSON 객체를 쉽게 생성할 수 있는 매크로
    Value, // 동적 JSON 값 타입 (구조를 모르는 JSON 데이터 처리용)
};
use thiserror::Error; // 에러 처리를 쉽게 만들어주는 매크로

// ✨ 요청 핸들러 함수
pub async fn handler(
    // WithRejection: 기존 Json<Value> 추출기를 감싸는 래퍼
    // - Json 파싱 실패 시 JsonRejection을 ApiError로 변환해줌
    //
    // 두 번째 파라미터(_)는 WithRejection 내부에 필요하지만
    // 핸들러 로직에서는 사용하지 않으므로 무시해도 됩니다.
    WithRejection(Json(value), _): WithRejection<Json<Value>, ApiError>,
) -> impl IntoResponse {
    // 받은 JSON 데이터를 디버깅 출력 후, 그대로 JSON 응답으로 반환
    Json(dbg!(value))
}

// ✨ 커스텀 에러 타입 정의
// - `thiserror` 매크로를 이용하여 `From<JsonRejection>` 변환을 자동 생성
#[derive(Debug, Error)]
pub enum ApiError {
    // `#[from]` 덕분에 JsonRejection → ApiError 변환이 자동으로 처리됩니다.
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
}

// ✨ ApiError → HTTP 응답 변환 구현
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        // 에러 종류에 따라 상태 코드와 메시지를 분기 처리
        let (status, message) = match self {
            ApiError::JsonExtractorRejection(json_rejection) => {
                (json_rejection.status(), json_rejection.body_text())
            }
        };

        // JSON 형태의 에러 응답 생성
        let payload = json!({
            "message": message,
            "origin": "with_rejection", // 에러가 발생한 위치(파일명)를 명시
        });

        // (상태 코드, JSON) 형태로 최종 응답 반환
        (status, Json(payload)).into_response()
    }
}
