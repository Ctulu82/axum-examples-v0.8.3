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
    collections::HashMap, // 키-값 쌍을 저장하는 해시맵
    sync::{
        atomic::{AtomicU64, Ordering}, // 원자적 u64 값 (ID 자동 증가용)
        Arc,
        Mutex, // 스레드 안전한 공유 메모리
    },
};

// -- ✨ 외부 라이브러리(axum, tower-http 등) 임포트
use axum::{
    extract::{
        rejection::JsonRejection, // 요청 본문(JSON) 파싱 실패 시 반환되는 에러 타입
        FromRequest,              // 커스텀 요청 추출기 정의를 위한 트레이트
        MatchedPath,              // 라우터에서 매칭된 경로 정보를 제공하는 추출기
        Request,                  // HTTP 요청(Request) 객체
        State,                    // 요청 처리 핸들러에 앱 상태(AppState)를 주입할 때 사용
    },
    http::StatusCode,                   // HTTP 상태 코드(200, 404, 500 등) 상수 정의
    response::{IntoResponse, Response}, // 핸들러 반환 타입을 HTTP 응답으로 변환하는 트레이트와 실제 응답 타입
    routing::post,                      // POST 메서드용 라우터 빌더
    Router,                             // 라우트들을 모아서 앱을 구성하는 메인 객체
};

use serde::{
    Deserialize, // serde를 이용해 JSON ↔ Rust struct 변환을 위한 역직렬화
    Serialize,   // serde를 이용해 JSON ↔ Rust struct 변환을 위한 직렬화
};

use time_library::Timestamp; // 외부 모듈: 시간 관련 데이터 구조체
use tower_http::trace::TraceLayer; // HTTP 요청/응답 트레이싱 미들웨어
use tracing_subscriber::{
    layer::SubscriberExt,    // 트레이싱 구독자 설정 도우미
    util::SubscriberInitExt, //
};

// -- ✨ 메인 함수

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
        .route("/users", post(users_create)) // POST /users 요청 → users_create 핸들러 연결
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &Request| {
                    // 각 요청마다 트레이싱 span 설정
                    let method = req.method();
                    let uri = req.uri();
                    let matched_path = req
                        .extensions()
                        .get::<MatchedPath>()
                        .map(|matched| matched.as_str());

                    tracing::debug_span!("request", %method, %uri, matched_path)
                })
                .on_failure(()), // 실패 시 기본 5xx 에러 로깅 비활성화 (커스텀 처리 예정)
        )
        .with_state(state); // 앱 상태(AppState)를 공유

    // ✨ 127.0.0.1:3000 포트에서 TCP 소켓 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 처리합니다.

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리합니다.
}

/// 📦 상태 및 도메인 모델 정의

// ✨ 앱의 글로벌 상태 구조체
#[derive(Default, Clone)]
struct AppState {
    next_id: Arc<AtomicU64>,               // 유저 ID 자동 증가 (스레드 안전)
    users: Arc<Mutex<HashMap<u64, User>>>, // 유저 목록 (공유 가능한 뮤텍스)
}

// ✨ 클라이언트에서 받아오는 JSON 요청 구조체
#[derive(Deserialize)]
struct UserParams {
    name: String, // 유저 이름
}

// ✨ 서버가 응답할 유저 데이터 구조체
#[derive(Serialize, Clone)]
struct User {
    id: u64,               // 유저 ID
    name: String,          // 유저 이름
    created_at: Timestamp, // 생성 시각 (외부 라이브러리 타입)
}

/// 🔄 사용자 생성 핸들러 및 JSON 래퍼 정의

// ✨ POST /users 요청을 처리하는 핸들러
async fn users_create(
    State(state): State<AppState>,        // 공유 상태(AppState) 추출
    AppJson(params): AppJson<UserParams>, // 요청 본문을 UserParams로 추출
) -> Result<AppJson<User>, AppError> {
    // ID 증가
    let id = state.next_id.fetch_add(1, Ordering::SeqCst);

    // 현재 시간 생성 (실패 가능성 있음)
    let created_at = Timestamp::now()?; // 실패하면 AppError::TimeError로 변환

    let user = User {
        id,
        name: params.name,
        created_at,
    };

    // 유저를 상태에 저장
    state.users.lock().unwrap().insert(id, user.clone());

    // 성공적으로 생성된 유저를 JSON 응답
    Ok(AppJson(user))
}

/// 🧩 커스텀 JSON 추출기 및 응답 타입

// ✨ AppJson: Json 추출 및 응답 처리를 위한 래퍼 타입
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))] // 추출 실패 시 AppError 사용
struct AppJson<T>(T);

// ✨ AppJson을 HTTP 응답으로 변환하는 로직
impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// 🚨 에러 타입 정의 및 처리

// ✨ 앱 전용 에러 타입
enum AppError {
    JsonRejection(JsonRejection),   // JSON 파싱 실패
    TimeError(time_library::Error), // 시간 생성 실패
}

// ✨ 에러를 HTTP 응답으로 변환하는 로직
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String, // 에러 메시지
        }

        // 에러에 따른 상태 코드 및 메시지 설정
        let (status, message) = match self {
            AppError::JsonRejection(rejection) => {
                // 사용자의 잘못된 입력
                (rejection.status(), rejection.body_text())
            }
            AppError::TimeError(err) => {
                // 서버 내부 오류 (클라이언트에 자세한 오류 내용 노출 금지)
                tracing::error!(%err, "error from time_library");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong".to_owned(),
                )
            }
        };

        // 에러 응답 반환
        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

// ✨ JSON 파싱 실패 → AppError로 변환
impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

// ✨ 시간 생성 실패 → AppError로 변환
impl From<time_library::Error> for AppError {
    fn from(error: time_library::Error) -> Self {
        Self::TimeError(error)
    }
}

/// ⏱️ 외부 라이브러리 시뮬레이션 (time_library)

// ✨ 시간 관련 외부 모듈 (모의)
mod time_library {
    use serde::Serialize;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Serialize, Clone)]
    pub struct Timestamp(u64); // u64 기반 Timestamp

    impl Timestamp {
        // 현재 시간을 생성 (실패할 수도 있음)
        pub fn now() -> Result<Self, Error> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);

            // 테스트를 위해 일부러 주기적으로 실패
            if COUNTER.fetch_add(1, Ordering::SeqCst) % 3 == 0 {
                Err(Error::FailedToGetTime)
            } else {
                Ok(Self(1337)) // 고정된 시간값 반환
            }
        }
    }

    // 시간 생성 실패 에러 정의
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
