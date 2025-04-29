//! 이 예제는 `FromRequest` 트레잇을 수동으로 구현하여
//! 커스텀 추출기(`Json<T>`)를 만들고,
//! 실패 시 더욱 풍부한 정보를 담은 에러 응답을 생성하는 방법을 보여줍니다.
//!
//! ✅ 장점: 추출기 실행 전/후의 전체 흐름을 완벽하게 제어할 수 있음 (async/await 지원)
//! ❎ 단점: 반복 코드(boilerplate)와 복잡도가 증가함

// --- axum 관련 모듈 임포트 ---
use axum::{
    extract::{
        rejection::JsonRejection, // JSON 추출 실패 시 발생하는 에러 타입
        FromRequest,              // 커스텀 추출기를 만들기 위한 트레잇
        MatchedPath,              // 요청 매칭된 경로(path) 정보를 추출하기 위한 타입
        Request,                  // HTTP 요청 본문 및 메타데이터
    },
    http::StatusCode,       // HTTP 상태 코드 (예: 200 OK, 400 Bad Request 등)
    response::IntoResponse, // 핸들러 반환값을 HTTP 응답으로 변환하는 트레잇
    RequestPartsExt,        // Request를 parts로 분리하거나 extract 기능을 추가해주는 트레잇
};

// --- serde_json 관련 모듈 임포트 ---
use serde_json::{
    json,  // JSON 객체를 쉽게 생성할 수 있는 매크로
    Value, // 동적 JSON 값 타입 (구조를 모르는 JSON 데이터 처리용)
};

// ✨ 요청 핸들러 함수
// 우리가 만든 커스텀 Json<T> 추출기를 사용
pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value)); // 입력 값을 로그로 출력하고, 다시 응답으로 반환
}

/// 🧩 커스텀 추출기 수동 구현

/// ✨ Json<T> 추출기 구조체 정의
/// - 기존 `axum::Json<T>`을 감싸는 래퍼 타입
pub struct Json<T>(pub T);

/// ✨ `FromRequest` 트레잇 수동 구현
impl<S, T> FromRequest<S> for Json<T>
where
    // 내부적으로 `axum::Json<T>`를 사용
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync, // 상태(state) 공유를 위한 trait 제약
{
    // 실패 시 반환할 타입 지정 (에러 응답용)
    type Rejection = (StatusCode, axum::Json<Value>);

    // 실제 추출 처리 함수
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // 요청을 parts/body 로 분해 (추출기 실행 전, 사전작업이 가능하게 함)
        let (mut parts, body) = req.into_parts();

        // ✨ 경로 정보 추출 (MatchedPath)
        // - 요청된 경로를 에러 응답에 포함시키기 위해 먼저 추출
        let path = parts
            .extract::<MatchedPath>() // 경로 정보 추출 시도
            .await
            .map(|path| path.as_str().to_owned()) // 성공 시 문자열로 변환
            .ok(); // 실패해도 무시하고 Option<String> 형태로 저장

        // 분해했던 parts와 body를 다시 합쳐서 원래 Request로 복원
        let req = Request::from_parts(parts, body);

        // ✨ 실제 Json<T> 추출 시도
        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)), // 성공하면 Json<T> 래퍼로 감싸서 반환
            Err(rejection) => {
                // ✨ 실패 시: 커스텀 에러 응답 생성
                let payload = json!({
                    "message": rejection.body_text(), // 원래 에러 메시지 텍스트
                    "origin": "custom_extractor",     // 커스텀 추출기임을 명시
                    "path": path,                     // 요청 경로 정보 (Optional)
                });

                // (HTTP 상태코드, JSON 에러 객체) 형태로 반환
                Err((rejection.status(), axum::Json(payload)))
            }
        }
    }
}
