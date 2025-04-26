//! axum 프레임워크의 기본 구조와 라우팅, JSON 요청/응답 처리 방법을 잘 보여주는 아주 전형적인 “README 스타일” 예제.
//!
//! ```not_rust
//! cargo run -p example-readme
//! ```

use axum::{
    http::StatusCode,       // HTTP 상태 코드 정의
    response::IntoResponse, // 핸들러 반환 타입
    routing::{get, post},   // get, post: HTTP GET, POST 요청용 라우터 생성 함수
    Json,                   // Json: 요청 또는 응답을 JSON 형태로 처리
    Router,                 // axum::Router: 라우팅을 구성하는 핵심 객체
};
use serde::{
    Deserialize, // serde를 이용해 JSON ↔ Rust struct 변환을 위한 역직렬화
    Serialize,   // serde를 이용해 JSON ↔ Rust struct 변환을 위한 직렬화
};

/// 🧵 메인 함수
#[tokio::main]
async fn main() {
    // 로깅/디버깅 출력을 위한 트레이싱 초기화
    tracing_subscriber::fmt::init();

    // 라우터 생성: GET `/`, POST `/users` 라우트를 등록
    let app = Router::new()
        .route("/", get(root)) // GET / 요청은 root 핸들러로 연결
        .route("/users", post(create_user)); // POST /users 요청은 create_user 핸들러로 연결

    // 127.0.0.1:3000 포트에서 TCP 소켓 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await // 비동기적으로 대기합니다.
        .unwrap(); // 에러 발생 시 패닉(panic) 처리합니다.

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app)
        .await // 비동기적으로 실행합니다.
        .unwrap(); // 에러 발생 시 패닉 처리합니다.
}

/// 📡 GET 핸들러
async fn root() -> &'static str {
    // 브라우저나 클라이언트가 / 경로로 접근하면 "Hello, World!" 응답
    "Hello, World!"
}

/// 👤 POST 핸들러
/// 클라이언트가 /users 경로로 JSON 형태의 POST 요청을 보내면:
async fn create_user(
    // 요청 본문을 JSON으로 파싱하여 `CreateUser` 타입으로 변환
    Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
    // 받은 username을 이용해 새로운 User 생성
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // (201 Created, JSON 응답) 형태로 반환
    (StatusCode::CREATED, Json(user))
}

// -- 📦 구조체 정의

// 클라이언트가 보낼 JSON 요청 형식
// 예: { "username": "taehyun" }
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// 서버가 응답할 JSON 형식
// 예: { "id": 1337, "username": "taehyun" }
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}

// ✅ 테스트 방법 예시
//
// # GET 요청
// curl http://127.0.0.1:3000/
// # → Hello, World!
//
// # POST 요청
/*
curl -X POST http://127.0.0.1:3000/users \
     -H 'Content-Type: application/json' \
     -d '{"username": "taehyun"}'
*/
// # → {"id":1337,"username":"taehyun"}
