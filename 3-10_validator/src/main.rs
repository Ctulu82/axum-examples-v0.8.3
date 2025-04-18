//! Run with
//!
//! ```not_rust
//! cargo run -p example-validator
//!
//! curl '127.0.0.1:3000?name='
//! -> Input validation error: [name: Can not be empty]
//!
//! curl '127.0.0.1:3000?name=LT'
//! -> <h1>Hello, LT!</h1>
//! ```

use axum::{
    extract::{rejection::FormRejection, Form, FromRequest, Request},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use serde::{de::DeserializeOwned, Deserialize};
use thiserror::Error;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use validator::Validate;

// 비동기로 동작하는 main 함수
#[tokio::main]
async fn main() {
    // 로깅 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 라우터가 들어있는 app 생성
    let app = app();

    // TCP 소켓 바인딩 (127.0.0.1:3000) 후 서버 실행
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// 실제로 라우터를 정의하는 함수
fn app() -> Router {
    // GET "/" 요청이 handler 함수로 연결
    Router::new().route("/", get(handler))
}

// 사용자로부터 들어올 파라미터 구조체
#[derive(Debug, Deserialize, Validate)]
pub struct NameInput {
    // validator 사용하여 길이가 2 이상이어야 함
    #[validate(length(min = 2, message = "Can not be empty"))]
    pub name: String,
}

// handler: /?name=... 라는 형태의 쿼리 파라미터를 입력 받는다.
// ValidatedForm<NameInput>를 통해 검증 완료된 데이터를 받음
async fn handler(ValidatedForm(input): ValidatedForm<NameInput>) -> Html<String> {
    Html(format!("<h1>Hello, {}!</h1>", input.name))
}

// ValidatedForm: 폼 입력을 받고, 자동으로 validator를 실행하는 구조체 래퍼
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedForm<T>(pub T);

// FromRequest 트레이트를 구현하여 Axum이 요청을 받을 때 자동으로 이 과정을 거치게 함
impl<T, S> FromRequest<S> for ValidatedForm<T>
where
    T: DeserializeOwned + Validate, // T는 Deserialize와 Validate 트레이트를 모두 구현해야 함
    S: Send + Sync,
    Form<T>: FromRequest<S, Rejection = FormRejection>,
{
    // 에러가 발생하면 ServerError로 감쌀 것이므로 Rejection 타입을 ServerError로 설정
    type Rejection = ServerError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Form<T>를 통해 요청 데이터를 파싱
        let Form(value) = Form::<T>::from_request(req, state).await?;
        // validator를 사용하여 검증
        value.validate()?;
        // 검증이 성공하면 ValidatedForm에 감싸서 반환
        Ok(ValidatedForm(value))
    }
}

// 서버 실행 중 발생 가능한 에러를 하나로 묶은 Enum
#[derive(Debug, Error)]
pub enum ServerError {
    // validator::ValidationErrors를 투명하게 래핑
    #[error(transparent)]
    ValidationError(#[from] validator::ValidationErrors),

    // Axum에서 Form 파싱 실패 시 발생할 수 있는 FormRejection을 래핑
    #[error(transparent)]
    AxumFormRejection(#[from] FormRejection),
}

// 에러를 HTTP 응답으로 바꿔주는 로직
impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            // ValidationError가 발생하면 상태 코드 400과 에러 메시지
            ServerError::ValidationError(_) => {
                let message = format!("Input validation error: [{self}]").replace('\n', ", ");
                (StatusCode::BAD_REQUEST, message)
            }
            // FormRejection 등 다른 폼 파싱 오류도 상태 코드 400 반환
            ServerError::AxumFormRejection(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        }
        .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // 테스트에서 body를 문자열로 받아오는 도움함수
    async fn get_html(response: Response<Body>) -> String {
        let body = response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    // name 파라미터가 전혀 없는 경우
    #[tokio::test]
    async fn test_no_param() {
        let response = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let html = get_html(response).await;
        assert_eq!(html, "Failed to deserialize form: missing field `name`");
    }

    // name 파라미터는 있지만 값이 없는 경우
    #[tokio::test]
    async fn test_with_param_without_value() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/?name=")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let html = get_html(response).await;
        assert_eq!(html, "Input validation error: [name: Can not be empty]");
    }

    // name 파라미터는 있으나 2글자 미만인 경우
    #[tokio::test]
    async fn test_with_param_with_short_value() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/?name=X")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let html = get_html(response).await;
        assert_eq!(html, "Input validation error: [name: Can not be empty]");
    }

    // name 파라미터가 2글자 이상 정상 입력인 경우
    #[tokio::test]
    async fn test_with_param_and_value() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/?name=LT")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let html = get_html(response).await;
        assert_eq!(html, "<h1>Hello, LT!</h1>");
    }
}
