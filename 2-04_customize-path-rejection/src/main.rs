//! 경로 파라미터(Path Params) 추출 시 발생하는 에러를 커스터마이징하는 예제입니다.
//! 예: /users/{user_id}/teams/{team_id} 요청에서
//! - 숫자가 아닌 값이 들어올 경우 -> '/users/foo/teams/10'
//! - 파라미터 개수가 맞지 않을 경우 -> '/users/1'
//! - UTF-8 오류 발생 등
//! 에 대해 명확하고 구조화된 JSON 에러 응답을 제공합니다.

use axum::{
    extract::{path::ErrorKind, rejection::PathRejection, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // ✨ tracing 설정: 로그 출력 및 레벨 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ✨ 라우터 구성: 커스텀 Path 추출기 적용
    let app = Router::new().route("/users/{user_id}/teams/{team_id}", get(handler));

    // ✨ 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// ✅ 핸들러 및 경로 파라미터 추출 구조체

// ✨ 커스텀 Path 추출기를 사용하는 핸들러
async fn handler(Path(params): Path<Params>) -> impl IntoResponse {
    axum::Json(params) // 추출된 파라미터를 JSON으로 응답
}

// ✨ 요청 경로에서 추출할 파라미터 구조체
#[derive(Debug, Deserialize, Serialize)]
struct Params {
    user_id: u32,
    team_id: u32,
}

/// 🧩 커스텀 Path 추출기 정의 및 구현

// ✨ 우리가 만든 커스텀 Path 추출기
struct Path<T>(T);

// ✨ 수동으로 FromRequestParts 트레잇 구현
impl<S, T> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send, // 역직렬화 가능한 타입
    S: Send + Sync,
{
    type Rejection = (StatusCode, axum::Json<PathError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(value) => Ok(Self(value.0)), // 정상 추출 시 그대로 반환

            Err(rejection) => {
                // ✨ 에러 종류에 따라 상태코드와 메시지 구성
                let (status, body) = match rejection {
                    PathRejection::FailedToDeserializePathParams(inner) => {
                        let mut status = StatusCode::BAD_REQUEST;

                        let kind = inner.into_kind(); // 상세 에러 종류 추출

                        let body = match &kind {
                            ErrorKind::WrongNumberOfParameters { .. } => PathError {
                                message: kind.to_string(),
                                location: None,
                            },

                            ErrorKind::ParseErrorAtKey { key, .. } => PathError {
                                message: kind.to_string(),
                                location: Some(key.clone()),
                            },

                            ErrorKind::ParseErrorAtIndex { index, .. } => PathError {
                                message: kind.to_string(),
                                location: Some(index.to_string()),
                            },

                            ErrorKind::ParseError { .. } => PathError {
                                message: kind.to_string(),
                                location: None,
                            },

                            ErrorKind::InvalidUtf8InPathParam { key } => PathError {
                                message: kind.to_string(),
                                location: Some(key.clone()),
                            },

                            ErrorKind::UnsupportedType { .. } => {
                                // 내부 버그성 오류 → 500 반환
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

/// 🔁 에러 메시지 구조체

#[derive(Serialize)]
struct PathError {
    message: String,          // 에러 메시지 내용
    location: Option<String>, // 어느 파라미터에서 오류 발생했는지 (예: "user_id")
}
