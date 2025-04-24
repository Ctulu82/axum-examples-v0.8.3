//! 실행 방법:
//!
//! ```not_rust
//! cargo run -p example-simple-router-wasm
//! ```
//!
//! 이 예제는 WASM 환경에서 axum을 사용하는 방식을 보여줍니다.
//! `wasm32-unknown-unknown` 타겟으로도 항상 컴파일 가능해야 합니다.
//! 즉, Axum을 WASM 환경에서 라우팅 없이 직접 함수로 사용하는 간단한 구조를 보여주는 흥미로운 예제!
//!
//! tokio의 I/O 레이어인 `mio`는 wasm 환경을 지원하지 않기 때문에,
//! 이 예제는 `axum`에 대해 `default-features = false` 설정이 필요합니다.
//!
//! 대부분의 서버리스 런타임은 단일 요청(Request)을 받아 단일 응답(Response)을 반환하는
//! 방식으로 작동합니다. 이는 axum의 `Handler` 트레잇과 유사합니다.
//!
//! 이 예제에서는 `main`이 서버리스 런타임처럼 작동하며 `app` 함수에 요청을 전달합니다.
//! `app` 함수는 axum 라우터를 통해 요청을 처리합니다.
//!
//! 🧠 예제 개요
//!  > axum을 일반적인 HTTP 서버로 사용하지 않고, WebAssembly 환경의 함수형 호출 스타일로 동작하게 설계됨.
//!  > 즉, 하나의 요청을 받아 하나의 응답을 반환하는 서버리스 함수 또는 클라우드 함수 구조를 흉내냄.

use axum::{
    response::{Html, Response},
    routing::get,
    Router,
};
use futures_executor::block_on; // async → sync 전환을 위한 executor
                                // async 코드를 sync로 실행. WASM 환경에서 이런 방식이 종종 필요.

use http::Request;
use tower_service::Service; // axum의 .call() 사용을 위한 트레잇

fn main() {
    // WASM 환경을 가정하여, HTTP 요청을 직접 코드에서 생성
    let request: Request<String> = Request::builder()
        .uri("https://serverless.example/api/") // 경로는 "/api/"
        .body("Some Body Data".into()) // 요청 본문
        .unwrap();

    // block_on 을 통해 async 함수인 `app`을 동기 방식으로 실행
    let response: Response = block_on(app(request));

    // 상태 코드가 200 OK인지 확인 (정상 동작 테스트용)
    assert_eq!(200, response.status());
}

// serverless 함수처럼 작동할 핸들러 함수
#[allow(clippy::let_and_return)]
async fn app(request: Request<String>) -> Response {
    // axum 라우터를 생성하고 "/api/" 경로에 GET 요청을 등록
    // Router::new().route(...): 서버리스 환경에서도 axum 라우팅 사용 가능
    let mut router = Router::new().route("/api/", get(index));

    // 요청을 라우터에 전달하여 응답을 받아옴
    let response = router.call(request).await.unwrap();
    response
}

// 실제 요청을 처리할 핸들러
async fn index() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>") // HTML 응답 return
}

// ✅ 실제 사용 예시

// 이 코드는 다음과 같은 환경에서 유용할 수 있습니다:
// 	•	클라우드플레어 Workers 또는 Vercel Edge Functions
// 	•	WASM + WASI 기반 런타임
// 	•	Function-as-a-Service (FaaS) 백엔드

// 🔍 실제 WASM으로 빌드해도 실행은?

// 1. wasm32-unknown-unknown 타겟으로 컴파일 ✅
// rustup target add wasm32-unknown-unknown
// cargo build --target wasm32-unknown-unknown --no-default-features
// → 이 예제는 컴파일만 가능하며,
// → 실행은 브라우저 또는 WASM 런타임 (wasmer, wasmtime 등) 에서 별도로 구현해야 합니다.
// 2. 실행은 불가능 ❌ (단독 실행 불가)
// wasm32-unknown-unknown 타겟은 main()을 실행할 수 있는 런타임이 없기 때문에,
// 실제로 실행하려면 JS 환경에서 WebAssembly 모듈로 불러와야 합니다.

// 🧪 테스트하고 싶다면?
// 이 예제는 “WASM처럼 작동하는 구조를 일반 Rust 코드로 구현”한 것이므로:
// 	•	cargo run 으로 테스트하면 충분합니다.
// 	•	실제 WASM 으로 배포하고 싶다면 wasm-bindgen 등의 JS bridge 를 설정해야 합니다.

// 🚀 참고: 정말 WASM으로 배포하고 싶다면?
// cargo install wasm-pack
// wasm-pack build --target web
// 이후 JS 코드로 해당 .wasm 바이너리를 import 해서 호출하는 구조로 이어집니다.
