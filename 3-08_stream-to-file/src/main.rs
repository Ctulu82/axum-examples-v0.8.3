//! Axum에서 HTTP 요청의 바디(파일 데이터)를 스트림 형태로 받아서 디스크에 저장하는 방법을 보여주는 실용적인 예제
//!
//! ```not_rust
//! cargo run -p example-stream-to-file
//! ```

use axum::{
    body::Bytes,
    extract::{Multipart, Path, Request},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
    BoxError, Router,
};
use futures::{Stream, TryStreamExt};
use std::io;
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 업로드된 파일을 저장할 디렉토리 이름 정의
const UPLOADS_DIRECTORY: &str = "uploads";

/// 🏁 main 함수

#[tokio::main]
async fn main() {
    // 로그 출력 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // save files to a separate directory to not override files in the current directory
    // 최초 실행 시 uploads/ 폴더가 없다면 생성 (없으면 저장 시 에러 발생)
    tokio::fs::create_dir(UPLOADS_DIRECTORY)
        .await
        .expect("failed to create `uploads` directory");

    let app = Router::new()
        .route("/", get(show_form).post(accept_form)) // HTML form
        .route("/file/{file_name}", post(save_request_body)); // raw body 업로드

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// 폼이 아닌 단순 스트림 형태의 POST 요청을 저장하는 Handler
// POST'ing to `/file/foo.txt` will create a file called `foo.txt`.
async fn save_request_body(
    Path(file_name): Path<String>,
    request: Request,
) -> Result<(), (StatusCode, String)> {
    // 바디는 .into_body().into_data_stream()을 통해 스트림으로 변환하여 저장
    stream_to_file(&file_name, request.into_body().into_data_stream()).await
}

// GET 요청 → 업로드 폼 출력 Handler
async fn show_form() -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Upload something!</title>
            </head>
            <body>
                <form action="/" method="post" enctype="multipart/form-data">
                    <div>
                        <label>
                            Upload file:
                            <input type="file" name="file" multiple>
                        </label>
                    </div>

                    <div>
                        <input type="submit" value="Upload files">
                    </div>
                </form>
            </body>
        </html>
        "#,
    )
}

// Handler that accepts a multipart form upload and streams each field to a file.
// POST 요청 (Multipart)
// 업로드된 multipart/form-data의 각 파일 필드를 하나씩 읽어 저장
async fn accept_form(mut multipart: Multipart) -> Result<Redirect, (StatusCode, String)> {
    while let Ok(Some(field)) = multipart.next_field().await {
        // field.file_name()이 존재할 경우만 저장
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };

        // 저장
        stream_to_file(&file_name, field).await?;
    }

    Ok(Redirect::to("/"))
}

// 💾 파일 저장 함수
// S: Stream<Item = Result<Bytes, E>> 형식의 스트림을 받아 파일로 저장
async fn stream_to_file<S, E>(path: &str, stream: S) -> Result<(), (StatusCode, String)>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    if !path_is_valid(path) {
        return Err((StatusCode::BAD_REQUEST, "Invalid path".to_owned()));
    }

    async {
        // Convert the stream into an `AsyncRead`.
        let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));

        // 내부에서 StreamReader로 AsyncRead처럼 다루고 tokio::io::copy()로 직접 디스크에 기록
        let body_reader = StreamReader::new(body_with_io_error);

        futures::pin_mut!(body_reader);

        // Create the file. `File` implements `AsyncWrite`.
        let path = std::path::Path::new(UPLOADS_DIRECTORY).join(path);
        let mut file = BufWriter::new(File::create(path).await?);

        // Copy the body into the file.
        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

// to prevent directory traversal attacks we ensure the path consists of exactly one normal
// component
// ✅ 경로 유효성 검증
// 디렉토리 탈출 방지 (../../../etc/passwd 같은 공격 차단)
// 경로는 반드시 1개의 “normal component”여야 함
// 예: foo.txt ✅
// 예: ../foo.txt, /etc/passwd, a/b/c.txt [❌]
fn path_is_valid(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek() {
        if !matches!(first, std::path::Component::Normal(_)) {
            return false;
        }
    }

    components.count() == 1
}
