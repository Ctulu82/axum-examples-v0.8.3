//! Example of application using <https://github.com/launchbadge/sqlx>
//!
//! 이 예제는 단순히 SELECT 'hello world from pg'만 실행하므로 실제 테이블은 없어도 됨.
//!
//! 🧠 언제 SQLx를 쓰면 좋을까?
//!  -> 🚀 비동기 성능이 중요한 서버 (Tokio 기반)
//!  -> 🔧 ORM 없이 직접 SQL을 다루고 싶을 때
//!  -> ✅ 안정성 있고 문서가 잘 되어 있음
//!  -> 📦 마이그레이션도 지원 (sqlx-cli)
//!
//! ```not_rust
//! curl 127.0.0.1:3000
//! curl -X POST 127.0.0.1:3000
//! ```

use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    routing::get,
    Router,
};
use sqlx::postgres::{PgPool, PgPoolOptions}; // sqlx의 PostgreSQL 연결 타입
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use std::time::Duration;

/// 🧭 main 함수: 서버 실행 & DB 풀 초기화

#[tokio::main]
async fn main() {
    // 로그 출력 설정 (tracing)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 데이터베이스 연결 문자열 읽기
    let db_connection_str = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:thisispassword@localhost".to_string());

    // SQLx의 비동기 PostgreSQL 커넥션 풀 생성
    let pool = PgPoolOptions::new()
        .max_connections(5) // 최대 연결 수
        .acquire_timeout(Duration::from_secs(3)) // 연결 타임아웃
        .connect(&db_connection_str)
        .await
        .expect("can't connect to database");

    // 라우터 설정
    let app = Router::new()
        .route(
            "/",
            get(using_connection_pool_extractor) // GET / 핸들러
                .post(using_connection_extractor), // POST / 핸들러
        )
        .with_state(pool); // 공유 상태로 PgPool 등록

    // 서버 바인딩 및 실행
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// 🧪 GET 핸들러: State에서 커넥션 풀 추출
async fn using_connection_pool_extractor(
    State(pool): State<PgPool>, // State(pool)에서 직접 커넥션 풀을 추출
) -> Result<String, (StatusCode, String)> {
    // query_scalar는 문자열 하나만 가져올 때 유용
    sqlx::query_scalar("select 'hello world from pg'") // 스칼라 값 하나만 추출
        .fetch_one(&pool) // fetch_one()은 쿼리 결과 단일 행을 가져옴
        .await
        .map_err(internal_error)
}

/// 🧱 커스텀 추출기 정의 (POST용)

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
struct DatabaseConnection(sqlx::pool::PoolConnection<sqlx::Postgres>);

impl<S> FromRequestParts<S> for DatabaseConnection
where
    PgPool: FromRef<S>, // State로부터 PgPool을 가져오는 기능
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = PgPool::from_ref(state);

        let conn = pool.acquire().await.map_err(internal_error)?; // 커넥션 한 개 획득

        Ok(Self(conn))
    }
}

/// 🧪 POST 핸들러: 커넥션 추출기 사용
async fn using_connection_extractor(
    DatabaseConnection(mut conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    sqlx::query_scalar("select 'hello world from pg'")
        .fetch_one(&mut *conn)
        .await
        .map_err(internal_error)
    // → 커스텀 추출기 덕분에 State를 명시하지 않아도 됨
}

/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
/// 🧯 공통 에러 핸들러
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

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
