//! 이 예제는 요청 처리 중 발생할 수 있는 다양한 에러(JSON 파싱 실패, 외부 라이브러리 오류 등)를
//! 커스텀 에러 타입으로 처리하고, HTTP 응답에 적절히 변환하는 방법을 보여줍니다.
//! > POST 시 JSON을 다음과 같이 세팅합니다. {"name":"string value"}
//! ! 3번 시도 시 한번은 에러로 떨어지도록 설계되었습니다.
//!
//! 실행 방법:
//!
//! ```bash
//! cargo run -p example-error-handling
//! ```

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use time_library::Timestamp;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // ✨ 로그 필터 및 포맷 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // 환경변수에서 로깅 레벨을 설정하지 않은 경우 기본값 적용
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer()) // 콘솔에 로그 출력
        .init();

    // ✨ 앱 상태 초기화
    let state = AppState::default();

    // ✨ 라우터 구성
    let app = Router::new()
        .route("/users", post(users_create)) // POST /users 경로
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request| {
                    // 로그용 트레이싱 span 설정
                    let method = req.method();
                    let uri = req.uri();
                    let matched_path = req
                        .extensions()
                        .get::<MatchedPath>()
                        .map(|matched| matched.as_str());

                    tracing::debug_span!("request", %method, %uri, matched_path)
                })
                .on_failure(()), // 기본 5xx 로깅은 생략 (커스텀 로깅을 사용하므로)
        )
        .with_state(state); // 상태 주입

    // ✨ 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// 📦 상태 및 도메인 모델 정의

// ✨ 앱의 글로벌 상태 정의
#[derive(Default, Clone)]
struct AppState {
    next_id: Arc<AtomicU64>,               // 유저 ID 자동 증가
    users: Arc<Mutex<HashMap<u64, User>>>, // 유저 목록 저장소
}

// ✨ 클라이언트로부터 받는 입력 구조체 (JSON 파싱 대상)
#[derive(Deserialize)]
struct UserParams {
    name: String,
}

// ✨ 응답용 유저 구조체
#[derive(Serialize, Clone)]
struct User {
    id: u64,
    name: String,
    created_at: Timestamp, // 외부 라이브러리 제공 타입
}

/// 🔄 사용자 생성 라우트 및 커스텀 JSON 추출기

// ✨ POST /users 요청 처리 핸들러
async fn users_create(
    State(state): State<AppState>,
    // 커스텀 JSON 추출기 사용
    AppJson(params): AppJson<UserParams>,
) -> Result<AppJson<User>, AppError> {
    let id = state.next_id.fetch_add(1, Ordering::SeqCst);

    // 외부 라이브러리 호출 시 오류 가능성 있음
    let created_at = Timestamp::now()?; // Result → AppError::TimeError로 변환됨

    let user = User {
        id,
        name: params.name,
        created_at,
    };

    // 유저 저장
    state.users.lock().unwrap().insert(id, user.clone());

    // JSON으로 응답
    Ok(AppJson(user))
}

/// 🧩 커스텀 JSON 추출기와 응답 변환

// ✨ AppJson: Json 추출기 및 응답 타입 래퍼
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))] // 실패 시 AppError 반환
struct AppJson<T>(T);

// ✨ 응답으로 변환 가능하게 구현
impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// 🚨 공통 에러 타입 정의 및 응답 구현

// ✨ 앱에서 발생 가능한 에러들을 열거
enum AppError {
    JsonRejection(JsonRejection),   // JSON 파싱 실패
    TimeError(time_library::Error), // 외부 라이브러리 오류
}

// ✨ 에러를 HTTP 응답으로 변환
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        // 상태코드 및 메시지를 결정
        let (status, message) = match self {
            AppError::JsonRejection(rejection) => {
                // 사용자 입력 오류 → 그대로 반환 (로깅은 생략)
                (rejection.status(), rejection.body_text())
            }
            AppError::TimeError(err) => {
                // 내부 오류는 로그로 기록 (클라이언트에 상세 정보 제공하지 않음)
                tracing::error!(%err, "error from time_library");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong".to_owned(),
                )
            }
        };

        // 에러 메시지를 JSON으로 응답
        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

// ✨ JSON 파싱 실패 → AppError로 자동 변환
impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

// ✨ 외부 에러 → AppError로 자동 변환
impl From<time_library::Error> for AppError {
    fn from(error: time_library::Error) -> Self {
        Self::TimeError(error)
    }
}

/// ⏱️ 외부 라이브러리 시뮬레이션 (time_library)

// 외부 라이브러리 시뮬레이션
mod time_library {
    use serde::Serialize;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Serialize, Clone)]
    pub struct Timestamp(u64);

    impl Timestamp {
        pub fn now() -> Result<Self, Error> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);

            // 세 번 중 한 번은 일부러 실패 (테스트용)
            if COUNTER.fetch_add(1, Ordering::SeqCst) % 3 == 0 {
                Err(Error::FailedToGetTime)
            } else {
                Ok(Self(1337)) // 고정값
            }
        }
    }

    #[derive(Debug)]
    pub enum Error {
        FailedToGetTime,
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "failed to get time")
        }
    }
}
