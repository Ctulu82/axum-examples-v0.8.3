use axum::{routing::post, Json, Router};
use serde_json::Value;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,            // 응답을 gzip, br 등으로 압축
    decompression::RequestDecompressionLayer, // 요청이 압축되어 있을 경우 자동 해제
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🧪 테스트 구조
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    // 로그 설정 초기화: 환경변수 RUST_LOG=example-compression=trace 가능
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 애플리케이션 라우터 구성
    let app: Router = app();

    // 127.0.0.1:3000 에서 수신 대기
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // 서버 실행
    axum::serve(listener, app).await.unwrap();
}

/// 📦 app() 함수
fn app() -> Router {
    Router::new()
        .route("/", post(root)) // POST / → root 핸들러로 연결
        .layer(
            ServiceBuilder::new()
                // 1️⃣ 요청이 압축(gzip 등)되어 있으면 자동으로 해제
                .layer(RequestDecompressionLayer::new())
                // 2️⃣ 응답을 클라이언트가 요청한 방식으로 압축
                .layer(CompressionLayer::new()),
        )
}

/// 🧾 핸들러 root
async fn root(Json(value): Json<Value>) -> Json<Value> {
    // JSON body 를 그대로 echo 하듯 응답
    Json(value)
}
