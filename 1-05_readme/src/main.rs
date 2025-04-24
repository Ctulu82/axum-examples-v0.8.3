//! axum 프레임워크의 기본 구조와 라우팅, JSON 요청/응답 처리 방법을 잘 보여주는 아주 전형적인 “README 스타일” 예제.
//!
//! ```not_rust
//! cargo run -p example-readme
//! ```

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json,   // Json: 요청 또는 응답을 JSON 형태로 처리
    Router, // axum::Router: 라우팅을 구성하는 핵심 객체
};
use serde::{Deserialize, Serialize}; // serde: JSON ↔ Rust struct 변환을 위한 직렬화/역직렬화 라이브러리

/// 🧵 메인 함수

#[tokio::main]
async fn main() {
    // 로깅/디버깅 출력을 위한 트레이싱 초기화
    tracing_subscriber::fmt::init();

    // 라우터 생성: GET `/`과 POST `/users` 라우트 추가
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user));

    // 127.0.0.1:3000 포트에서 TCP 소켓 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // hyper 기반 서버 실행
    axum::serve(listener, app).await.unwrap();
}

/// 📡 GET 핸들러
async fn root() -> &'static str {
    // 브라우저나 클라이언트가 / 경로로 접근하면 "Hello, World!" 응답
    "Hello, World!"
}

/// 👤 POST 핸들러
/// 클라이언트가 /users 경로로 JSON 형태의 POST 요청을 보내면:
async fn create_user(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    // JSON payload를 CreateUser 구조체로 파싱
    Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
    // insert your application logic here
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // this will be converted into a JSON response
    // with a status code of `201 Created`
    // 응답은 (201 Created, JSON 응답) 형태로 반환
    (StatusCode::CREATED, Json(user))
}

// -- 📦 구조체 정의

// the input to our `create_user` handler
// 클라이언트에서 보낸 JSON 요청 형식
// 예시: { "username": "taehyun" }
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// the output to our `create_user` handler
// 응답 시 서버가 반환하는 JSON 형식
// 예시: { "id": 1337, "username": "taehyun" }
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
