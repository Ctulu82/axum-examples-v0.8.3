//! Redis + bb8 커넥션 풀 + axum의 실전 통합 예제
//!
//! ✅ 예제 개요
//! • Redis에 연결
//! • bb8 커넥션 풀로 Redis 연결 관리
//! • axum 핸들러에서 커넥션 풀을 사용하는 2가지 방법
//! • 핸들러 내에서 Redis get("foo") 요청 처리
//! • Redis에 사전 set("foo", "bar") 수행
//!
//! ```not_rust
//! cargo run -p example-tokio-redis
//! ```

// Axum 관련 모듈 임포트
use axum::{
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    routing::get,
    Router,
};

// Redis 비동기 연결 풀 관련 모듈
use bb8::{Pool, PooledConnection};
use bb8_redis::bb8; // bb8::Pool 등의 접근을 위해 필요
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands; // Redis 명령어 trait
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🚀 main() 함수
#[tokio::main]
async fn main() {
    // 로깅 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Redis 연결 매니저 생성 및 커넥션 풀 구성
    tracing::debug!("connecting to redis");
    let manager = RedisConnectionManager::new("redis://localhost").unwrap();
    let pool = bb8::Pool::builder().build(manager).await.unwrap();

    {
        // ping the database before starting
        // Redis 연결 테스트: foo = bar 설정 및 검증
        let mut conn = pool.get().await.unwrap();
        conn.set::<&str, &str, ()>("foo", "bar").await.unwrap();
        let result: String = conn.get("foo").await.unwrap();
        assert_eq!(result, "bar");
    }

    tracing::debug!("successfully connected to redis and pinged it");

    // build our application with some routes
    // 라우터 설정: GET, POST 둘 다 지원
    let app = Router::new()
        .route(
            "/",
            get(using_connection_pool_extractor) // 방식 1: State로 직접 풀 추출
                .post(using_connection_extractor), // 방식 2: 커스텀 추출기 사용
        )
        .with_state(pool); // 상태(State)로 Redis 커넥션 풀 제공

    // 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// 🧪 방식 1: State<ConnectionPool> 추출기

type ConnectionPool = Pool<RedisConnectionManager>;

async fn using_connection_pool_extractor(
    State(pool): State<ConnectionPool>, // 상태에서 풀을 추출
) -> Result<String, (StatusCode, String)> {
    let mut conn = pool.get().await.map_err(internal_error)?; // 풀에서 커넥션 얻기
    let result: String = conn.get("foo").await.map_err(internal_error)?; // Redis에서 값 가져오기
    Ok(result)
}

// 🧪 방식 2: 커스텀 추출기 DatabaseConnection

// we can also write a custom extractor that grabs a connection from the pool
// which setup is appropriate depends on your application
// 커스텀 추출기 정의
struct DatabaseConnection(PooledConnection<'static, RedisConnectionManager>);

// FromRequestParts 구현
impl<S> FromRequestParts<S> for DatabaseConnection
where
    ConnectionPool: FromRef<S>, // 상태에서 풀을 추출할 수 있어야 함
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = ConnectionPool::from_ref(state);

        let conn = pool.get_owned().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}

async fn using_connection_extractor(
    DatabaseConnection(mut conn): DatabaseConnection,
) -> Result<String, (StatusCode, String)> {
    let result: String = conn.get("foo").await.map_err(internal_error)?;

    Ok(result)
}

/// 🛠 에러 처리 헬퍼
/// Utility function for mapping any error into a `500 Internal Server Error`
/// response.
fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

// 🧪 테스트 방법
//
// 1.	Redis 서버 실행:
// redis-server
//
// redis-cli ping => PONG
//
// 2.	서버 실행:
// cargo run -p example-tokio-redis
//
// 3.	curl 요청 확인:
// curl http://localhost:3000/
// # 결과: bar
// curl -X POST http://localhost:3000/
// # 결과: bar
//
// 종료
// redis-cli shutdown
