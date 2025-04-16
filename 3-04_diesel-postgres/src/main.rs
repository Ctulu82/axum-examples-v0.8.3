//! Run with
//!
//! ```not_rust
//! cargo run -p example-diesel-postgres
//! ```
//!
//! Checkout the [diesel webpage](https://diesel.rs) for
//! longer guides about diesel
//!
//! Checkout the [crates.io source code](https://github.com/rust-lang/crates.io/)
//! for a real world application using axum and diesel

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 디젤 마이그레이션을 바이너리에 포함시키는 매크로
// migrations/ 디렉토리 내의 SQL 마이그레이션들을 embed해서 바이너리 실행 시 바로 적용할 수 있게 함.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

// 🏗️ Diesel 테이블 선언 (macro)
// normally part of your generated schema.rs file
// Diesel은 매크로를 사용해서 이 테이블 정보를 기반으로 ORM을 생성합니다.
table! {
    users (id) {
        id -> Integer,
        name -> Text,
        hair_color -> Nullable<Text>,
    }
}

/// ✅ 모델 정의

// DB에서 읽은 데이터를 응답으로 직렬화
#[derive(serde::Serialize, Selectable, Queryable)]
struct User {
    id: i32,
    name: String,
    hair_color: Option<String>,
}

// 클라이언트에서 입력받아 삽입
#[derive(serde::Deserialize, Insertable)]
#[diesel(table_name = users)]
struct NewUser {
    name: String,
    hair_color: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv().ok(); // .env 파일 로드

    // 예: postgres://postgres:thisispassword@localhost/testdb
    let db_url = std::env::var("DATABASE_URL").unwrap();

    // Diesel + Deadpool 기반 풀 생성
    let manager = deadpool_diesel::postgres::Manager::new(db_url, deadpool_diesel::Runtime::Tokio1);
    let pool = deadpool_diesel::postgres::Pool::builder(manager)
        .build()
        .unwrap();

    // 📌 서버 실행 시 마이그레이션(up.sql)이 자동 실행
    {
        let conn = pool.get().await.unwrap();
        conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(|_| ()))
            .await
            .unwrap()
            .unwrap();
    }

    // 🧪 라우팅 및 핸들러
    let app = Router::new()
        .route("/user/list", get(list_users))
        .route("/user/create", post(create_user))
        .with_state(pool);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// ✏️ POST /user/create
async fn create_user(
    State(pool): State<deadpool_diesel::postgres::Pool>,
    Json(new_user): Json<NewUser>,
) -> Result<Json<User>, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(users::table)
                .values(new_user)
                .returning(User::as_returning()) // PostgreSQL 전용 반환
                .get_result(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    Ok(Json(res))
}

/// 🔍 GET /user/list
async fn list_users(
    State(pool): State<deadpool_diesel::postgres::Pool>,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    let res = conn
        .interact(|conn| users::table.select(User::as_select()).load(conn))
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    Ok(Json(res))
}

/// 🔥 에러 헬퍼: 어떤 에러든 500 Internal Server Error로 매핑
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

// 🧪 예시 요청 (Postman)
//
// POST /user/create
// { "name": "Alice", "hair_color": "black" }
//
// GET /user/list
//[ { "id": 1, "name": "Alice", "hair_color": "black" } ]

// PostgreSQL 설치
// $ brew install postgresql
//
// Homebrew로 libpq 설치
// $ brew install libpq
//
// 빌드 시 libpq 관련 문제 생길 경우 cargo clean && cargo build
//
// PostgreSQL 서비스 시작
// $ brew services start postgresql
//
// PostgreSQL 서비스 중지
// $ brew services stop postgresql
//
// PostgreSQl 콘솔로 접속하기
// $ psql postgres
//
// 사용자 확인
// postgres=# \du
//
// 사용자 생성 (예제 실행 전 최초 1회 필수!)
// > postgres라는 유저명, password를 'thisispassword'로 설정하여 유저 생성
// CREATE ROLE postgres WITH LOGIN PASSWORD 'thisispassword'
//
// 테이블 생성 (예제 실행 전 최초 1회 필수!)
// CREATE DATABASE testdb OWNER postgres;
