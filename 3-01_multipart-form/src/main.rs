//! Multipart 추출기를 사용하여 브라우저에서 업로드된 파일을 multipart/form-data 형식으로 처리.
//! HTML 폼에서 파일 여러 개를 선택하여 업로드하고, 서버에서 그 내용을 읽어 로그로 출력하는 구조.

use axum::{
    extract::{DefaultBodyLimit, Multipart}, // Multipart 폼 데이터 추출기
    response::Html,                         // HTML 반환용 응답 타입
    routing::get,
    Router,
};
use tower_http::limit::RequestBodyLimitLayer; // 바디 용량 제한 설정용 미들웨어
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // tracing 로그 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 🧱 앱 라우터 설정
    let app = Router::new()
        // "/" 경로에 GET: 폼 보여주기 / POST: 폼 제출 처리
        .route("/", get(show_form).post(accept_form))
        // ✨ 기본 요청 바디 제한을 해제 (Axum 기본값: 2MB)
        .layer(DefaultBodyLimit::disable())
        // ✨ 요청 바디 최대 크기 제한: 250MB
        .layer(RequestBodyLimitLayer::new(
            250 * 1024 * 1024, /* 250mb */
        ))
        // ✨ 요청 추적 로그
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // 🚀 hyper로 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// 🧾 GET 요청: 파일 업로드 폼 보여주기

// enctype="multipart/form-data"는 폼을 파일 업로드용으로 설정합니다.
// multiple 속성으로 여러 파일을 한 번에 업로드할 수 있게 됩니다.
async fn show_form() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head></head>
            <body>
                <form action="/" method="post" enctype="multipart/form-data">
                    <label>
                        Upload file:
                        <input type="file" name="file" multiple> <!-- 다중 파일 업로드 -->
                    </label>

                    <input type="submit" value="Upload files">
                </form>
            </body>
        </html>
        "#,
    )
}

/// 📩 POST 요청: 업로드된 파일 처리

async fn accept_form(mut multipart: Multipart) {
    // multipart.next_field() 로 순차적으로 각 필드를 가져옵니다.
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string(); // 필드 이름
        let file_name = field.file_name().unwrap().to_string(); // 업로드된 파일 이름
        let content_type = field.content_type().unwrap().to_string(); // MIME 타입
        let data = field.bytes().await.unwrap(); // 파일 바이트 전체 읽기

        // 업로드된 파일 정보 출력
        println!(
            "Length of `{name}` (`{file_name}`: `{content_type}`) is {} bytes",
            data.len()
        );
    }
}
