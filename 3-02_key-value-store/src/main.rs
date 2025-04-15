//! 인메모리 기반의 키-값 저장소를 Axum 기반으로 구현한 예제입니다.
//!
//! ```bash
//! cargo run -p example-key-value-store
//! ```

use axum::{
    body::Bytes,                      // 요청/응답 바디의 바이너리
    error_handling::HandleErrorLayer, // 미들웨어 에러 핸들링
    extract::{DefaultBodyLimit, Path, State},
    handler::Handler, // .post_service() 사용을 위한 트레잇
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
    Router,
};

use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, RwLock}, // 공유 상태를 위한 RwLock
    time::Duration,
};

use tower::{BoxError, ServiceBuilder};
use tower_http::{
    compression::CompressionLayer, limit::RequestBodyLimitLayer, trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 🧭 main 함수: 서버 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let shared_state = SharedState::default();

    // 🧱 라우터 구성
    let app = Router::new()
        // 키 경로에 대해 GET/POST 제공
        .route(
            "/{key}",
            get(
                // 응답 압축 적용
                kv_get.layer(CompressionLayer::new()),
            )
            .post_service(
                // POST 처리용 서비스 핸들러
                kv_set
                    .layer((
                        DefaultBodyLimit::disable(),              // 기본 바디 제한 해제
                        RequestBodyLimitLayer::new(1024 * 5_000), // 최대 5MB로 제한
                    ))
                    .with_state(Arc::clone(&shared_state)), // 상태 주입
            ),
        )
        // 저장된 키 리스트 조회
        .route("/keys", get(list_keys))
        // 관리자 전용 경로 `/admin` 하위에 포함
        .nest("/admin", admin_routes())
        // 전역 미들웨어
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error)) // 미들웨어 에러 핸들링
                .load_shed() // 과부하 처리
                .concurrency_limit(1024) // 동시 처리 제한
                .timeout(Duration::from_secs(10)) // 요청당 10초 제한
                .layer(TraceLayer::new_for_http()), // 요청 추적 로그
        )
        .with_state(Arc::clone(&shared_state));

    // 🌐 hyper 로 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// 📦 상태 정의
type SharedState = Arc<RwLock<AppState>>;

#[derive(Default)]
struct AppState {
    db: HashMap<String, Bytes>,
}

/// 📩 핸들러 함수들

// 🔍 GET /{key}
async fn kv_get(
    Path(key): Path<String>,
    State(state): State<SharedState>,
) -> Result<Bytes, StatusCode> {
    let db = &state.read().unwrap().db;

    if let Some(value) = db.get(&key) {
        Ok(value.clone())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// ✏️ POST /{key}
async fn kv_set(Path(key): Path<String>, State(state): State<SharedState>, bytes: Bytes) {
    state.write().unwrap().db.insert(key, bytes);
}

// 🗂️ GET /keys
async fn list_keys(State(state): State<SharedState>) -> String {
    let db = &state.read().unwrap().db;

    db.keys()
        .map(|key| key.to_string())
        .collect::<Vec<String>>()
        .join("\n")
}

// 🔐 관리자 API (/admin 하위)
fn admin_routes() -> Router<SharedState> {
    async fn delete_all_keys(State(state): State<SharedState>) {
        state.write().unwrap().db.clear();
    }

    async fn remove_key(Path(key): Path<String>, State(state): State<SharedState>) {
        state.write().unwrap().db.remove(&key);
    }

    Router::new()
        .route("/keys", delete(delete_all_keys)) // DELETE /admin/keys
        .route("/key/{key}", delete(remove_key)) // DELETE /admin/key/{key}
        .layer(ValidateRequestHeaderLayer::bearer("secret-token")) // Bearer 인증 적용
}

// 🚨 에러 핸들링
async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}

// 🧪 요청 예시
//
// 데이터 저장
// > curl -X POST http://localhost:3000/mykey -d 'hello axum!'
//
// 데이터 조회
// > curl http://localhost:3000/mykey
//
// 모든 키 목록 조회
// > curl http://localhost:3000/keys
//
// 관리자 - 모든 데이터 삭제
// > curl -X DELETE http://localhost:3000/admin/keys \
// >   -H "Authorization: Bearer secret-token"
