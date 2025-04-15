//! MongoDB와 연동되는 Axum 기반의 간단한 회원 API 서버입니다.
//!
//! POST /create   → 회원 생성
//! GET  /read/{id} → 회원 조회
//! PUT  /update   → 회원 수정
//! DELETE /delete/{id} → 회원 삭제

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};

use mongodb::{
    bson::doc,
    results::{DeleteResult, InsertOneResult, UpdateResult},
    Client, Collection,
};

use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 🧭 DB 연결 & 서버 실행

    // MongoDB 연결 문자열 (환경변수 또는 기본값 사용)
    let db_connection_str = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "mongodb://admin:password@127.0.0.1:27017/?authSource=admin".to_string()
    });

    // MongoDB 클라이언트 생성
    let client = Client::with_uri_str(db_connection_str).await.unwrap();

    // DB 연결 테스트: ping 커맨드 실행
    client
        .database("axum-mongo")
        .run_command(doc! { "ping": 1 })
        .await
        .unwrap();

    println!("Pinged your database. Successfully connected to MongoDB!");

    // 📋 로깅 미들웨어 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 🚀 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app(client)).await.unwrap();
}

// 🔧 라우터 정의 함수
fn app(client: Client) -> Router {
    // members 컬렉션 선택
    let collection: Collection<Member> = client.database("axum-mongo").collection("members");

    Router::new()
        .route("/create", post(create_member))
        .route("/read/{id}", get(read_member))
        .route("/update", put(update_member))
        .route("/delete/{id}", delete(delete_member))
        .layer(TraceLayer::new_for_http()) // 로그 추적 미들웨어
        .with_state(collection) // 콜렉션을 핸들러에 주입
}

/// ✅ 핸들러 함수들

// POST /create – 신규 회원 생성
async fn create_member(
    State(db): State<Collection<Member>>,
    Json(input): Json<Member>,
) -> Result<Json<InsertOneResult>, (StatusCode, String)> {
    let result = db.insert_one(input).await.map_err(internal_error)?;

    Ok(Json(result))
}

// GET /read/{id} – 특정 ID 조회
async fn read_member(
    State(db): State<Collection<Member>>,
    Path(id): Path<u32>,
) -> Result<Json<Option<Member>>, (StatusCode, String)> {
    let result = db
        .find_one(doc! { "_id": id })
        .await
        .map_err(internal_error)?;

    Ok(Json(result))
}

// PUT /update – 기존 회원 수정 (전체 덮어쓰기 방식)
async fn update_member(
    State(db): State<Collection<Member>>,
    Json(input): Json<Member>,
) -> Result<Json<UpdateResult>, (StatusCode, String)> {
    let result = db
        .replace_one(doc! { "_id": input.id }, input)
        .await
        .map_err(internal_error)?;

    Ok(Json(result))
}

// DELETE /delete/{id} – 기존 회원 삭제
async fn delete_member(
    State(db): State<Collection<Member>>,
    Path(id): Path<u32>,
) -> Result<Json<DeleteResult>, (StatusCode, String)> {
    let result = db
        .delete_one(doc! { "_id": id })
        .await
        .map_err(internal_error)?;

    Ok(Json(result))
}

/// 🧠 에러 핸들러 함수
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

/// 📄 데이터 모델 정의
#[derive(Debug, Deserialize, Serialize)]
struct Member {
    #[serde(rename = "_id")] // MongoDB에서 기본 ID 필드는 `_id`
    id: u32,
    name: String,
    active: bool,
}

// 🧪 테스트 예시 (Postman or curl)
//
// 회원 생성 요청
// > curl -X POST http://localhost:3000/create \
// >.     -H "Content-Type: application/json" \
// >      -d '{"_id": 1, "name": "Alice", "active": true}'
//
// 🔍 회원 조회
// > curl http://localhost:3000/read/1
//
// 📝 회원 수정
// > curl -X PUT http://localhost:3000/update \
//        -H "Content-Type: application/json" \
//        -d '{"_id":1,"name":"Alice Updated","active":false}'
//
// ❌ 회원 삭제
// > curl -X DELETE http://localhost:3000/delete/1

// 서버 실행 전 MongoDB 설치 필수! (2025.04.15 기준)
// $ brew tap mongodb/brew
// $ brew install mongodb-community@6.0
// $ brew services start mongodb/brew/mongodb-community@6.0
//
// MongoDB 첫 설정
// $ mongosh
// use admin
// db.createUser({user: "admin",pwd: "password",roles: [ { role: "readWriteAnyDatabase", db: "admin" } ]})
//
// DB 중지 및 삭제
// $ brew services stop mongodb/brew/mongodb-community@6.0
//
// 실행 확인
// $ brew services list
