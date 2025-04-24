//! Run with
//!
//! ```not_rust
//! cargo run -p example-templates-minijinja
//! ```
//! MiniJinja 템플릿 엔진을 사용한 예제.
//! Python 진영에서 유명한 Jinja2와 거의 같은 문법을 가진 Rust용 템플릿 엔진으로,
//! Askama와 달리 런타임에 템플릿을 등록하고 사용할 수 있는 유연한 방식.

use axum::extract::State;
use axum::http::StatusCode;
use axum::{response::Html, routing::get, Router};
use minijinja::{context, Environment};
use std::sync::Arc;

/// 📦 앱 상태 정의 (템플릿 환경 포함)
struct AppState {
    env: Environment<'static>, // MiniJinja의 템플릿 저장소
                               // MiniJinja는 Environment에 템플릿을 등록하고 → 나중에 꺼내서 렌더링함
}

/// --- 🧠 main 함수

#[tokio::main]
async fn main() {
    // MiniJinja 환경 생성
    let mut env = Environment::new();

    // 템플릿 등록
    env.add_template("layout", include_str!("../templates/layout.jinja"))
        .unwrap();
    env.add_template("home", include_str!("../templates/home.jinja"))
        .unwrap();
    env.add_template("content", include_str!("../templates/content.jinja"))
        .unwrap();
    env.add_template("about", include_str!("../templates/about.jinja"))
        .unwrap();

    // pass env to handlers via state
    // Arc 상태로 공유 (라우터 핸들러들에 전달할 용도)
    let app_state = Arc::new(AppState { env });

    // 라우터 설정
    let app = Router::new()
        .route("/", get(handler_home)) // 홈 페이지
        .route("/content", get(handler_content)) // 콘텐츠 페이지
        .route("/about", get(handler_about)) // 소개 페이지
        .with_state(app_state); // 상태 공유

    // 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// --- 🚏 핸들러들 (라우팅 처리)

/// "/" → 홈
async fn handler_home(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    // 템플릿 호출
    let template = state.env.get_template("home").unwrap();

    let rendered = template
        .render(context! {  // context!{}: 템플릿에 넘겨줄 변수 설정
            title => "Home",
            welcome_text => "Hello World!",
        })
        .unwrap();

    // 응답으로 HTML 반환
    Ok(Html(rendered))
}

/// "/content" → 콘텐츠 목록
async fn handler_content(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    // 템플릿 호출
    let template = state.env.get_template("content").unwrap();

    // 템플릿 변수로 entries 리스트 전달
    let some_example_entries = vec!["Data 1", "Data 2", "Data 3"];

    let rendered = template
        .render(context! {  // context!{}: 템플릿에 넘겨줄 변수 설정
            title => "Content",
            entries => some_example_entries,
        })
        .unwrap();

    // 응답으로 HTML 반환
    Ok(Html(rendered))
}

/// "/about" → 소개 페이지
async fn handler_about(State(state): State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    // 템플릿 호출
    let template = state.env.get_template("about").unwrap();

    let rendered = template
        .render(context!{    // context!{}: 템플릿에 넘겨줄 변수 설정
            title => "About",
            about_text => "Simple demonstration layout for an axum project with minijinja as templating engine.",
    }).unwrap();

    // 응답으로 HTML 반환
    Ok(Html(rendered))
}

// 🧩 jinja 템플릿

// 	1.	layout.jinja → 공통 레이아웃 (HTML <head>, <nav>, {% block content %} 구조)
// 	2.	home.jinja → 홈 콘텐츠
// 	3.	content.jinja → 반복 리스트 처리
// 	4.	about.jinja → 설명 페이지

// ✅ 실행 테스트

// 브라우저에서 다음 경로를 오픈:
// 	http://localhost:3000/
// 	http://localhost:3000/content
// 	http://localhost:3000/about
