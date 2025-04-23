//! Axum 기반의 Server-Sent Events (SSE) 기능을 사용하는 예제
//! HTTP 연결을 유지하며 서버가 일방향으로 실시간 데이터를 push하는 구조를 보여줌.
//!
//! ```not_rust
//! cargo run -p example-sse
//! ```
//! 그다음 브라우저에서 http://localhost:3000 그리고 /sse 접속
//! 콘솔 로그에서 hi!, 그리고 브라우저 화면에서 keep-alive-text 메시지 수신 확인
//!
//! Test with
//! ```not_rust
//! cargo test -p example-sse
//! ```

use axum::{
    response::sse::{Event, Sse}, // Sse → Server Sent Events 형식의 응답
    routing::get,                // Event → 클라이언트로 보낼 단일 SSE 메시지 단위
    Router,
};
use axum_extra::TypedHeader; // TypedHeader → User-Agent 같은 HTTP 헤더 파싱
use futures::stream::{self, Stream};
use std::{convert::Infallible, path::PathBuf, time::Duration};
use tokio_stream::StreamExt as _;
use tower_http::{services::ServeDir, trace::TraceLayer}; // ServeDir → / 경로에 정적 HTML/JS 파일 제공
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// ✅ main 함수

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

    // 애플리케이션 정의 및 실행
    let app = app();

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// ✅ app() – 라우터 및 정적 파일 설정
fn app() -> Router {
    // 정적 파일은 assets/ 디렉토리에서 읽어옴
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let static_files_service = ServeDir::new(assets_dir).append_index_html_on_directories(true);

    // build our application with a route
    Router::new()
        .fallback_service(static_files_service) // / → index.html 서빙
        .route("/sse", get(sse_handler)) // /sse → SSE 응답 핸들러로 연결
        .layer(TraceLayer::new_for_http()) // 요청 트레이싱 미들웨어
}

/// ✅ sse_handler – SSE 이벤트 핸들러
/// 반환 타입은 Sse<Stream<...>> → SSE 방식으로 스트리밍 응답 전송
async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // 클라이언트의 User-Agent를 로그로 출력
    println!("`{}` connected", user_agent.as_str());

    // `Stream` 은 1초마다 이벤트를 반복함.
    // futures::stream::repeat_with()를 통해 Event("hi!")를 1초마다 전송
    let stream = stream::repeat_with(|| Event::default().data("hi!"))
        .map(Ok) // Result<Event, Infallible> 형식으로 변환
        .throttle(Duration::from_secs(1)); // throttle()은 전송 간격 조절

    // SSE 연결 유지(Connection: keep-alive)를 위해 1초 간격의 "keep-alive-text"를 보냄
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

#[cfg(test)]
mod tests {
    use eventsource_stream::Eventsource;
    use tokio::net::TcpListener;

    use super::*;

    /// ✅ integration_test – SSE 테스트 (옵션)
    ///    임시 서버를 띄워 /sse 엔드포인트로 요청을 보내고 "hi!" 메시지를 수신하는지 검증
    ///    eventsource_stream을 이용하여 SSE 응답 스트림을 처리
    ///    첫 메시지가 "hi!"인지 확인
    #[tokio::test]
    async fn integration_test() {
        // A helper function that spawns our application in the background
        async fn spawn_app(host: impl Into<String>) -> String {
            let host = host.into();
            // Bind to localhost at the port 0, which will let the OS assign an available port to us
            let listener = TcpListener::bind(format!("{}:0", host)).await.unwrap();
            // Retrieve the port assigned to us by the OS
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(async {
                axum::serve(listener, app()).await.unwrap();
            });

            // Returns address (e.g. http://127.0.0.1{random_port})
            format!("http://{}:{}", host, port)
        }

        let listening_url = spawn_app("127.0.0.1").await;

        let mut event_stream = reqwest::Client::new()
            .get(format!("{}/sse", listening_url))
            .header("User-Agent", "integration_test")
            .send()
            .await
            .unwrap()
            .bytes_stream()
            .eventsource()
            .take(1);

        let mut event_data: Vec<String> = vec![];
        while let Some(event) = event_stream.next().await {
            match event {
                Ok(event) => {
                    // break the loop at the end of SSE stream
                    if event.data == "[DONE]" {
                        break;
                    }

                    event_data.push(event.data);
                }
                Err(_) => {
                    panic!("Error in event stream");
                }
            }
        }

        assert!(event_data[0] == "hi!");
    }
}
