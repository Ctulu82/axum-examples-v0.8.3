//! 경로 파라미터(Path Params) 추출 시 발생하는 에러를 커스터마이징하는 예제입니다.
//!
//! 예: /users/{user_id}/teams/{team_id} 요청에서
//! - 숫자가 아닌 값이 들어올 경우 -> '/users/foo/teams/10'
//! - 파라미터 개수가 맞지 않을 경우 -> '/users/1'
//! - UTF-8 오류 발생 등
//!
//! 다양한 오류 상황에 대해 명확하고 구조화된 JSON 에러 응답을 제공합니다.

use axum::{
    extract::{
        path::ErrorKind,          // 경로 추출 에러 종류
        rejection::PathRejection, // 경로 추출 실패 리젝션
        FromRequestParts,         // 요청 파트에서 추출하는 트레잇
    },
    http::{
        request::Parts, // HTTP 요청 헤더와 메타데이터
        StatusCode,     // HTTP 상태 코드
    },
    response::IntoResponse, // 응답으로 변환하는 트레잇
    routing::get,           // GET 메서드 라우팅
    Router,                 // 라우터 객체
};

use serde::{
    de::DeserializeOwned, // 제네릭 역직렬화를 위한 트레잇
    Deserialize,          // Deserialize 매크로
    Serialize,            // Serialize 매크로
};

use tracing_subscriber::{
    layer::SubscriberExt,    // Layer 확장 기능
    util::SubscriberInitExt, // Subscriber 초기화 확장 기능
};

/// 💻 메인 함수

#[tokio::main]
async fn main() {
    // ✨ tracing 설정: 환경 변수 기반 필터와 포맷터를 등록하여 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ✨ 라우터 구성: 특정 경로에 핸들러 등록
    let app = Router::new().route("/users/{user_id}/teams/{team_id}", get(handler));

    // ✨ 서버 리스너 바인딩 및 시작
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 발생

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리
}

/// ✅ 핸들러 및 경로 파라미터 추출 구조체

// ✨ 커스텀 Path 추출기를 사용하는 핸들러
async fn handler(Path(params): Path<Params>) -> impl IntoResponse {
    axum::Json(params) // 추출한 파라미터를 JSON 형식으로 응답
}

// ✨ 요청 경로에서 추출할 파라미터를 정의한 구조체
#[derive(Debug, Deserialize, Serialize)]
struct Params {
    user_id: u32, // 사용자 ID
    team_id: u32, // 팀 ID
}

/// 🧩 커스텀 Path 추출기 정의 및 구현

// ✨ 사용자 정의 Path 추출기
struct Path<T>(T);

// ✨ 수동으로 FromRequestParts 트레잇 구현
impl<S, T> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send, // 역직렬화가 가능하고, 스레드 안전한 타입
    S: Send + Sync,             // 요청 상태도 스레드 안전해야 함
{
    type Rejection = (StatusCode, axum::Json<PathError>); // 실패 시 반환할 타입

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self(value.0)), // 정상 추출 시 값 반환

            Err(rejection) => {
                // ✨ 에러 종류에 따라 상태코드 및 에러 메시지 결정
                let (status, body) = match rejection {
                    PathRejection::FailedToDeserializePathParams(inner) => {
                        let mut status = StatusCode::BAD_REQUEST;

                        let kind = inner.into_kind(); // 상세 에러 정보 추출

                        let body = match &kind {
                            ErrorKind::WrongNumberOfParameters { .. } => PathError {
                                message: kind.to_string(),
                                location: None,
                            },

                            ErrorKind::ParseErrorAtKey { key, .. } => PathError {
                                message: kind.to_string(),
                                location: Some(key.clone()), // 특정 키에서 오류 발생
                            },

                            ErrorKind::ParseErrorAtIndex { index, .. } => PathError {
                                message: kind.to_string(),
                                location: Some(index.to_string()), // 특정 인덱스에서 오류 발생
                            },

                            ErrorKind::ParseError { .. } => PathError {
                                message: kind.to_string(),
                                location: None,
                            },

                            ErrorKind::InvalidUtf8InPathParam { key } => PathError {
                                message: kind.to_string(),
                                location: Some(key.clone()), // UTF-8 오류 발생한 키
                            },

                            ErrorKind::UnsupportedType { .. } => {
                                // 지원하지 않는 타입 요청 → 서버 내부 오류
                                status = StatusCode::INTERNAL_SERVER_ERROR;
                                PathError {
                                    message: kind.to_string(),
                                    location: None,
                                }
                            }

                            ErrorKind::Message(msg) => PathError {
                                message: msg.clone(),
                                location: None,
                            },

                            _ => PathError {
                                message: format!("Unhandled deserialization error: {kind}"),
                                location: None,
                            },
                        };

                        (status, body)
                    }

                    PathRejection::MissingPathParams(error) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PathError {
                            message: error.to_string(),
                            location: None,
                        },
                    ),

                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        PathError {
                            message: format!("Unhandled path rejection: {rejection}"),
                            location: None,
                        },
                    ),
                };

                Err((status, axum::Json(body)))
            }
        }
    }
}

/// 🔁 에러 메시지를 구조화하기 위한 구조체

#[derive(Serialize)]
struct PathError {
    message: String,          // 에러 메시지
    location: Option<String>, // 에러가 발생한 위치(키 또는 인덱스)
}
