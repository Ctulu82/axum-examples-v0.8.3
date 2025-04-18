//! Provides a RESTful web server managing some Todos.
//!
//! API will be:
//!
//! - `GET /todos`: return a JSON list of Todos.
//! - `POST /todos`: create a new Todo.
//! - `PATCH /todos/{id}`: update a specific Todo.
//! - `DELETE /todos/{id}`: delete a specific Todo.

use axum::{
    error_handling::HandleErrorLayer,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

/// 🏁 main()

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 빈 Todo 저장소 생성
    let db = Db::default();

    // Compose the routes
    let app = Router::new()
        .route("/todos", get(todos_index).post(todos_create))
        .route("/todos/{id}", patch(todos_update).delete(todos_delete))
        // Add middleware to all routes
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {error}"),
                        ))
                    }
                })) // 에러 핸들링 미들웨어
                .timeout(Duration::from_secs(10)) // 요청 타임아웃 설정
                .layer(TraceLayer::new_for_http()) // 요청/응답 로그 추적
                .into_inner(),
        )
        .with_state(db); // 공유 상태 등록

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// The query parameters for todos index
#[derive(Debug, Deserialize, Default)]
pub struct Pagination {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// 📚 라우트별 핸들러

// 1️⃣ GET /todos
// Query<Pagination>으로 페이징 지원 (offset, limit)
async fn todos_index(pagination: Query<Pagination>, State(db): State<Db>) -> impl IntoResponse {
    let todos = db.read().unwrap();

    let todos = todos
        .values()
        .skip(pagination.offset.unwrap_or(0)) // 전체 리스트에서 skip().take()로 범위 제한
        .take(pagination.limit.unwrap_or(usize::MAX))
        .cloned()
        .collect::<Vec<_>>();

    Json(todos) // JSON 형식으로 반환
}

#[derive(Debug, Deserialize)]
struct CreateTodo {
    text: String,
}

// 2️⃣ POST /todos
async fn todos_create(State(db): State<Db>, Json(input): Json<CreateTodo>) -> impl IntoResponse {
    let todo = Todo {
        id: Uuid::new_v4(), // 고유 ID 부여
        text: input.text,   // 클라이언트에서 받은 text 값으로 새로운 Todo 생성
        completed: false,
    };

    db.write().unwrap().insert(todo.id, todo.clone());

    // 반환 시 StatusCode::CREATED (201)과 JSON 함께 응답
    (StatusCode::CREATED, Json(todo))
}

#[derive(Debug, Deserialize)]
struct UpdateTodo {
    text: Option<String>,
    completed: Option<bool>,
}

// 3️⃣ PATCH /todos/{id}
async fn todos_update(
    Path(id): Path<Uuid>,
    State(db): State<Db>,
    Json(input): Json<UpdateTodo>,
) -> Result<impl IntoResponse, StatusCode> {
    // 기존 Todo를 읽고 일부 필드를 수정
    let mut todo = db
        .read()
        .unwrap()
        .get(&id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?; // 존재하지 않으면 404 Not Found

    if let Some(text) = input.text {
        todo.text = text;
    }

    if let Some(completed) = input.completed {
        todo.completed = completed;
    }

    db.write().unwrap().insert(todo.id, todo.clone());

    // 수정 후 다시 저장하고 JSON 반환
    Ok(Json(todo))
}

// 4️⃣ DELETE /todos/{id}
async fn todos_delete(Path(id): Path<Uuid>, State(db): State<Db>) -> impl IntoResponse {
    // ID 기반으로 삭제
    if db.write().unwrap().remove(&id).is_some() {
        StatusCode::NO_CONTENT // 성공 시 204 No Content
    } else {
        StatusCode::NOT_FOUND // 없으면 404 Not Found
    }
}

/// 📌 Db 타입 정의
/// rc<RwLock<...>> → 멀티 스레드 안전한 공유 상태
/// HashMap<Uuid, Todo> → ID별 Todo 저장소
/// 실무에서는 보통 DB 대체 용도로 쓰는 메모리 캐시 구조입니다.
type Db = Arc<RwLock<HashMap<Uuid, Todo>>>;

/// 📌 Todo 구조체
/// > Serialize → JSON 응답용
/// > Clone → 읽은 후 수정 시 다시 저장하기 위해 필요
#[derive(Debug, Serialize, Clone)]
struct Todo {
    id: Uuid,
    text: String,
    completed: bool,
}

// 🧪 테스트 예시 (Postman 또는 curl)
//
// ✅ 새 Todo 추가
// curl -X POST http://localhost:3000/todos -H 'Content-Type: application/json' \
// -d '{"text": "Buy milk"}'
//
// ✅ 전체 Todo 리스트 조회
// curl http://localhost:3000/todos
//
// ✅ Todo 업데이트
// curl -X PATCH http://localhost:3000/todos/<id> -H 'Content-Type: application/json' \
// -d '{"completed": true}'
//
// ✅ Todo 삭제
// curl -X DELETE http://localhost:3000/todos/<id>
//
// 🔒 참고: 실무 적용 시 고려사항
//  - 데이터 저장소: PostgreSQL, MongoDB 등 (예제는 메모리(HashMap))
//  - 인증 처리: JWT, OAuth (예제는 없음)
//  - 데이터 영속성: DB연동 필요 (예제는 없음)
//  - 동시성 충돌: 트랜잭션/락 관리 필요 (예제는 단순 RwLock)
//
// ✅ 요약
// 	 - Axum의 RESTful 구조 이해에 이상적인 예제
// 	 - 상태는 Arc<RwLock<HashMap<...>>>으로 관리
// 	 - 실무로 확장하려면 DB, 인증, 트랜잭션 처리 필요
