//! Run with
//!
//! ```sh
//! export DATABASE_URL=postgres://localhost/your_db
//! diesel migration run
//! cargo run -p example-diesel-async-postgres
//! ```
//!
//! Checkout the [diesel webpage](https://diesel.rs) for
//! longer guides about diesel
//!
//! Checkout the [crates.io source code](https://github.com/rust-lang/crates.io/)
//! for a real world application using axum and diesel

use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use diesel::prelude::*;
use diesel_async::{
    pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection, RunQueryDsl,
};
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 🏗️ Diesel 테이블 선언 (macro)
// normally part of your generated schema.rs file
// Diesel은 매크로를 사용해서 이 테이블 정보를 기반으로 ORM을 생성합니다.
// diesel-cli로 자동 생성 가능하며, 이 예제에서는 직접 선언되어 있음.
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

// API 요청(body)에서 받은 값을 DB에 삽입
#[derive(serde::Deserialize, Insertable)]
#[diesel(table_name = users)]
struct NewUser {
    name: String,
    hair_color: Option<String>,
}

type Pool = bb8::Pool<AsyncDieselConnectionManager<AsyncPgConnection>>;

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

    let db_url = std::env::var("DATABASE_URL").unwrap();

    // set up connection pool
    let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
    let pool = bb8::Pool::builder().build(config).await.unwrap();

    // 🛣️ 라우터 구성
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
    State(pool): State<Pool>,
    Json(new_user): Json<NewUser>,
) -> Result<Json<User>, (StatusCode, String)> {
    let mut conn = pool.get().await.map_err(internal_error)?;

    // Diesel + 비동기 연결을 이용한 삽입
    let res = diesel::insert_into(users::table)
        .values(new_user)
        .returning(User::as_returning())
        .get_result(&mut conn)
        .await
        .map_err(internal_error)?;
    Ok(Json(res))
}

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
struct DatabaseConnection(
    // 커넥션 풀에서 연결 가져오는 추출기
    bb8::PooledConnection<'static, AsyncDieselConnectionManager<AsyncPgConnection>>,
);

impl<S> FromRequestParts<S> for DatabaseConnection
where
    S: Send + Sync,
    Pool: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = Pool::from_ref(state);

        let conn = pool.get_owned().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
} // 이렇게 만들면 다른 핸들러에서 State(pool) 없이 DatabaseConnection만 선언해도 됩니다.

/// 🔍 GET /user/list
async fn list_users(
    DatabaseConnection(mut conn): DatabaseConnection,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    let res = users::table
        .select(User::as_select())
        .load(&mut conn)
        .await
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
