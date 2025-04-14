//! 이 예제는 Axum에서 CORS(Cross-Origin Resource Sharing)를 설정하는 방법을 보여줍니다.
//!
//! - localhost:3000 (프론트엔드 서버) 에서
//! - localhost:4000 (백엔드 서버)의 `/json` 엔드포인트로 fetch 요청을 보냅니다.
//!
//! 실행 명령어:
//! ```bash
//! cargo run -p example-cors
//! ```

use axum::{
    http::{HeaderValue, Method},    // CORS 설정 시 필요한 메서드/헤더 타입
    response::{Html, IntoResponse}, // HTML/응답 관련 타입
    routing::get,
    Json,
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer; // CORS 레이어

/// 🧭 메인 함수 – 프론트와 백엔드 동시 실행

#[tokio::main]
async fn main() {
    // ✨ 프론트엔드 서버 (포트 3000)
    let frontend = async {
        let app = Router::new().route("/", get(html));
        serve(app, 3000).await;
    };

    // ✨ 백엔드 API 서버 (포트 4000)
    let backend = async {
        let app = Router::new()
            .route("/json", get(json)) // JSON 응답용 경로
            .layer(
                // ✨ CORS 설정 적용
                CorsLayer::new()
                    // 이 출처(origin)에서 오는 요청만 허용
                    .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
                    // GET 요청만 허용 (기본은 아무 것도 허용 안됨)
                    .allow_methods([Method::GET]),
            );

        serve(app, 4000).await;
    };

    // ✨ 두 서버를 동시에 실행
    tokio::join!(frontend, backend);
}

/// 🧱 서버 실행 함수

// 주어진 포트에 앱을 바인딩하고 실행
async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// 🖥️ 프론트엔드 HTML → JS fetch 요청 포함

// 3000번 포트에서 제공되는 프론트엔드 페이지
async fn html() -> impl IntoResponse {
    // CORS 요청: 다른 포트(4000)의 백엔드에 요청
    Html(
        r#"
        <script>
            fetch('http://localhost:4000/json')
              .then(response => response.json())
              .then(data => console.log(data));
        </script>
        "#,
    )
}

/// 🧾 백엔드 응답

// 백엔드에서 JSON 배열을 응답
async fn json() -> impl IntoResponse {
    Json(vec!["one", "two", "three"])
}

// 🧪 동작 흐름 요약
// 흐름	설명
// 1	유저가 localhost:3000/ 접속 시 HTML과 JS를 받음 (127.0.01:3000 이 아님)
// 1-1  F12 로 콘솔 로그를 확인할 것.
// 2	JS 코드에서 localhost:4000/json 로 fetch 요청 발생
// 3	서버 간 출처(origin)가 다르므로 브라우저는 CORS preflight 검사 수행
// 4	CorsLayer를 통해 백엔드는 CORS 응답을 보내고 fetch 요청 허용
// 5	백엔드 JSON 응답이 프론트 콘솔에 출력됨
