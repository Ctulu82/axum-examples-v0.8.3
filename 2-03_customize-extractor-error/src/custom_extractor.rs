//! 이 예제는 `FromRequest` 트레잇을 수동으로 구현하여
//! 커스텀 추출기(`Json<T>`)를 만들고,
//! 실패 시 더욱 풍부한 정보를 담은 에러 응답을 생성하는 방법을 보여줍니다.
//!
//! ✅ 장점: 추출기 실행 전/후 흐름을 완전하게 제어 가능 (async/await 사용 가능)
//! ❌ 단점: 반복 코드(boilerplate)와 복잡도가 증가함

use axum::{
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Request},
    http::StatusCode,
    response::IntoResponse,
    RequestPartsExt, // parts.extract() 호출을 위한 트레잇
};
use serde_json::{json, Value};

// ✨ 요청 핸들러 함수
// 우리가 만든 커스텀 Json<T> 추출기를 사용
pub async fn handler(Json(value): Json<Value>) -> impl IntoResponse {
    Json(dbg!(value)); // 입력 값을 로그로 출력하고, 다시 응답으로 반환
}

/// 🧩 커스텀 추출기 수동 구현

// ✨ Json<T> 추출기 구조체 정의 (axum::Json<T>을 감싼 래퍼)
pub struct Json<T>(pub T);

// ✨ FromRequest 수동 구현
impl<S, T> FromRequest<S> for Json<T>
where
    // 내부적으로 axum::Json<T> 추출기를 사용하며, 그 Rejection 타입은 JsonRejection
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync, // 상태(state) 공유에 필요한 trait
{
    // 실패 시 반환할 타입 지정 (에러 응답용)
    type Rejection = (StatusCode, axum::Json<Value>);

    // 실제 추출 처리 함수
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // 요청을 parts/body 로 분해 (추출기 실행 전, 사전작업이 가능하게 함)
        let (mut parts, body) = req.into_parts();

        // ✨ 라우트 경로 정보 추출
        // Json 추출 전에 MatchedPath 를 먼저 추출해야 합니다.
        let path = parts
            .extract::<MatchedPath>() // 경로 정보 추출 시도
            .await
            .map(|path| path.as_str().to_owned())
            .ok(); // 실패해도 무시하고 Option<String> 으로 받음

        // parts와 body를 다시 합쳐서 원래 Request 로 복원
        let req = Request::from_parts(parts, body);

        // ✨ 실제 Json 추출 시도
        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)), // 정상 추출 시 Json<T>를 래핑하여 반환

            // ✨ 실패 시: 에러 메시지를 우리가 원하는 구조로 변환
            Err(rejection) => {
                let payload = json!({
                    "message": rejection.body_text(), // 원래 에러 메시지
                    "origin": "custom_extractor",     // 출처 정보
                    "path": path,                     // 요청 경로
                });

                Err((rejection.status(), axum::Json(payload)))
            }
        }
    }
}
