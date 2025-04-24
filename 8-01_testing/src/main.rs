//! Axum 애플리케이션의 다양한 테스트 방법을 보여주는 실용적인 예제
//!
//! ```not_rust
//! cargo test -p example-testing
//! ```

use std::net::SocketAddr;

use axum::{
    extract::ConnectInfo, // ConnectInfo: 요청한 클라이언트의 소켓 주소(IP:포트)를 가져올 수 있게 해줌.
    routing::{get, post},
    Json, // Axum의 주요 추출기.
    Router,
    // ServiceExt, // Axum의 주요 라우팅 도구.
};
use tower_http::trace::TraceLayer; // TraceLayer: 요청 로그 추적용 미들웨어.
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// --- 🔧 main()

#[tokio::main] // #[tokio::main] → 서버 실행 엔트리 포인트
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    // 서버는 127.0.0.1:3000 에서 실행되며, app() 으로 정의된 라우터를 사용
    axum::serve(
        listener,
        app().into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

/// Having a function that produces our app makes it easy to call it from tests
/// without having to create an HTTP server.
fn app() -> Router {
    Router::new()
        // / 라우트는 GET 요청에 대해 “Hello, World!” 문자열 반환
        .route("/", get(|| async { "Hello, World!" }))
        // /json 라우트는 POST 요청을 받고 JSON 을 감싸서 다시 반환함
        .route(
            "/json",
            post(|payload: Json<serde_json::Value>| async move {
                Json(serde_json::json!({ "data": payload.0 }))
            }),
        )
        // /requires-connect-info: 접속자의 IP 주소를 반환
        .route(
            "/requires-connect-info",
            get(|ConnectInfo(addr): ConnectInfo<SocketAddr>| async move { format!("Hi {addr}") }),
        )
        // 요청 추적용 미들웨어 적용
        .layer(TraceLayer::new_for_http())
}

/// --- 🧪 테스트 모듈

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::connect_info::MockConnectInfo,
        http::{self, Request, StatusCode},
    };
    use http_body_util::BodyExt; // for `collect`
    use serde_json::{json, Value};
    use tokio::net::TcpListener;
    use tower::{Service, ServiceExt}; // for `call`, `oneshot`, and `ready`

    /// 1. hello_world(): 기본 응답 확인
    #[tokio::test]
    async fn hello_world() {
        let app = app();

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            // app() 호출 후 테스트용 요청을 oneshot() 으로 보냄
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Hello, World!"); // 결과는 "Hello, World!"
    }

    /// 2. json(): JSON body 테스트
    #[tokio::test]
    async fn json() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/json")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!([1, 2, 3, 4])).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        // JSON [1,2,3,4] 을 전송하면 { "data": [1,2,3,4] } 반환
        assert_eq!(body, json!({ "data": [1, 2, 3, 4] }));
    }

    /// 3. not_found(): 존재하지 않는 라우트에 대한 테스트
    #[tokio::test]
    async fn not_found() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        // 존재하지 않는 라우트 /does-not-exist → 404 응답 확인
        assert!(body.is_empty());
    }

    /// 4. the_real_deal(): 실제 TCP 서버 바인딩 후 클라이언트로 테스트
    // You can also spawn a server and talk to it like any other HTTP server:
    #[tokio::test]
    async fn the_real_deal() {
        // 동적으로 포트를 바인딩하여 서버 시작 후
        let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app()).await.unwrap();
        });

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let response = client
            .request(
                Request::builder()
                    .uri(format!("http://{addr}"))
                    .header("Host", "localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        // hyper_util::client 로 실제 요청 전송 → “Hello, World!” 확인
        assert_eq!(&body[..], b"Hello, World!");
    }

    /// 5. multiple_request(): 여러 요청 테스트 (서비스 재사용)
    // You can use `ready()` and `call()` to avoid using `clone()`
    // in multiple request
    #[tokio::test]
    async fn multiple_request() {
        let mut app = app().into_service();

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = ServiceExt::<Request<Body>>::ready(&mut app)
            .await
            .unwrap()
            .call(request) // ready().call() 을 통해 여러 요청을 한 Router 인스턴스로 반복 전송
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = ServiceExt::<Request<Body>>::ready(&mut app)
            .await
            .unwrap()
            .call(request) // ready().call() 을 통해 여러 요청을 한 Router 인스턴스로 반복 전송
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// 6. with_into_make_service_with_connect_info(): ConnectInfo 테스트
    // Here we're calling `/requires-connect-info` which requires `ConnectInfo`
    //
    // That is normally set with `Router::into_make_service_with_connect_info` but we can't easily
    // use that during tests. The solution is instead to set the `MockConnectInfo` layer during
    // tests.
    #[tokio::test]
    async fn with_into_make_service_with_connect_info() {
        let mut app = app()
            // 일반적으로 서버가 셋업하는 ConnectInfo 를 모킹하여 직접 주입
            .layer(MockConnectInfo(SocketAddr::from(([0, 0, 0, 0], 3000))))
            .into_service();

        let request = Request::builder()
            .uri("/requires-connect-info")
            .body(Body::empty())
            .unwrap();
        let response = app.ready().await.unwrap().call(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
