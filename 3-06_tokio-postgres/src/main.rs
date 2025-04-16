//!
//! 이전에 diesel-postgres 예제에서 이미 유저 'postgres' 생성.
//! PostgreSQL은 인증 방법을 설정 파일에서 제어하므로 현재는 비밀번호 없이도 로컬에서 접속이 허용된 상태
//! 이 예제는 단순히 PostgreSQL 쿼리 연동이 되는지만 확인하는 헬로 월드 스타일의 테스트
//!

use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    routing::get,
    Router,
};
use bb8::{Pool, PooledConnection}; // 커넥션 풀과 개별 커넥션 타입
use bb8_postgres::PostgresConnectionManager; // bb8은 tokio-postgres용 풀을 지원.
use tokio_postgres::NoTls; // SSL 없는 접속을 위해 NoTls 사용.
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🔧 main() 함수

#[tokio::main]
async fn main() {
    // tracing 로그 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // PostgreSQL 비동기 연결 매니저 구성
    let manager =
        PostgresConnectionManager::new_from_stringlike("host=localhost user=postgres", NoTls)
            .unwrap();
    // user=postgres는 유저명, 패스워드가 없으면 trust 인증 설정이 필요할 수 있음

    // bb8 풀 빌더로 커넥션 풀 생성
    let pool = Pool::builder().build(manager).await.unwrap();

    // 🌐 Axum 라우터 설정
    let app = Router::new()
        .route(
            "/",
            get(using_connection_pool_extractor).post(using_connection_extractor),
            // GET  / → 상태 기반 풀 사용
            // POST / → 커스텀 추출기 사용
        )
        .with_state(pool);

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

type ConnectionPool = Pool<PostgresConnectionManager<NoTls>>;

/// 🧪 GET 핸들러 - 커넥션 풀 직접 사용
async fn using_connection_pool_extractor(
    State(pool): State<ConnectionPool>,
) -> Result<String, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;

    let row = conn
        .query_one("select 1 + 1", &[]) // query_one은 단일 행 반환
        .await
        .map_err(internal_error)?;
    let two: i32 = row.try_get(0).map_err(internal_error)?; // try_get(0)은 첫 번째 열의 값을 꺼냄

    // 최종 결과는 "2" 문자열 반환
    Ok(two.to_string())
}

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
// 🧱 커스텀 추출기 정의
// → DatabaseConnection을 추출기로 만들어 State 없이도 커넥션을 주입받게 함
struct DatabaseConnection(PooledConnection<'static, PostgresConnectionManager<NoTls>>);

/// FromRef<S> 제약 조건으로 Pool을 추출
/// 커넥션을 .get_owned()으로 비동기 획득
impl<S> FromRequestParts<S> for DatabaseConnection
where
    ConnectionPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = ConnectionPool::from_ref(state);

        let conn = pool.get_owned().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}

/// 🧪 POST 핸들러 - 추출기 기반
async fn using_connection_extractor(
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    let row = conn
        .query_one("select 1 + 1", &[])
        .await
        .map_err(internal_error)?;
    let two: i32 = row.try_get(0).map_err(internal_error)?;

    // → 동일하게 1 + 1 쿼리를 실행하여 "2" 응답
    Ok(two.to_string())
}

/// 💥 공통 에러 처리기
/// 모든 에러를 500 상태 코드로 포장하여 클라이언트에 전달
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

// 🧪 예시 요청 (브라우저 / Postman)
// > GET http://localhost:3000/ → 2
// > POST http://localhost:3000/ → 2

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
