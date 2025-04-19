//! Example OAuth (Discord) implementation.
//!
//! Discord OAuth2 인증 흐름을 구현한 예제로, 다음 절차를 따릅니다:
//!
//! 1) <https://discord.com/developers/applications>에서 애플리케이션 생성
//! 2) OAuth2 탭에서 CLIENT_ID, CLIENT_SECRET 확보
//!    1363140223600562327
//!    yQxHBQB9twUx4DIXXUZ4fO2fF_TOSUya
//! 3) 리다이렉션 URI에 `http://127.0.0.1:3000/auth/authorized` 추가
//! 4) 다음처럼 실행:
//! ```not_rust
//! CLIENT_ID=REPLACE_ME CLIENT_SECRET=REPLACE_ME cargo run -p example-oauth
//! ```
//! 엔드포인트 실행 순서 (Postman 말고 웹브라우저에서 실행할 것)
//! 01_ GET /auth/discord -> GET /auth/authorized (자동이라서 수동으로 실행하면 에러남)
//! 02_ GET / : index 페이지
//! 03_ GET /protected : 인증된 영역의 정보를 보여줌.
//! 04_ GET /logout (이후 protected 이동 시도하면 Discord 로 리다이렉트 됨.)
//!

use anyhow::{anyhow, Context, Result};
use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts, Query, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    RequestPartsExt, Router,
};
use axum_extra::{headers, typed_header::TypedHeaderRejectionReason, TypedHeader};
use http::{header, request::Parts, StatusCode};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, env};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 세션 저장소에 사용될 쿠키 이름
static COOKIE_NAME: &str = "SESSION";
/// CSRF 토큰 키 (세션 내부에서 사용)
static CSRF_TOKEN: &str = "csrf_token";

/// ✅ 서버 초기화 및 상태 구성
#[tokio::main]
async fn main() {
    // 로깅 초기화 (RUST_LOG or 기본 로그 필터)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 메모리 기반 세션 저장소 생성 (실제 서비스에선 Redis 등을 권장)
    let store = MemoryStore::new();

    // OAuth 클라이언트 구성 (CLIENT_ID, CLIENT_SECRET 등 환경변수 기반)
    let oauth_client = oauth_client().unwrap();

    // 앱 전체 상태 구성
    let app_state = AppState {
        store,
        oauth_client,
    };

    // 라우터 정의: 각 URL에 핸들러 연결 및 상태 주입
    let app = Router::new()
        .route("/", get(index)) // 인덱스 페이지 (사용자 정보 표시)
        .route("/auth/discord", get(discord_auth)) // Discord 인증 요청 (자동)
        .route("/auth/authorized", get(login_authorized)) // OAuth 콜백 처리
        .route("/protected", get(protected)) // 보호된 라우트
        .route("/logout", get(logout)) // 로그아웃
        .with_state(app_state); // 상태 주입

    // TCP 리스너 바인딩 및 서버 실행
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .context("failed to bind TcpListener")
        .unwrap();

    tracing::debug!(
        "listening on {}",
        listener
            .local_addr()
            .context("failed to return local address")
            .unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

/// 앱 전체에서 사용할 상태 구조체
#[derive(Clone)]
struct AppState {
    store: MemoryStore,        // 세션 저장소
    oauth_client: BasicClient, // OAuth2 클라이언트
}

/// `AppState`에서 `MemoryStore`를 추출하기 위한 구현
impl FromRef<AppState> for MemoryStore {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

/// `AppState`에서 `BasicClient`를 추출하기 위한 구현
impl FromRef<AppState> for BasicClient {
    fn from_ref(state: &AppState) -> Self {
        state.oauth_client.clone()
    }
}

/// ✅ OAuth 클라이언트 설정 함수
/// Discord OAuth2 서버와 통신할 수 있도록 `BasicClient`를 설정합니다.
/// 환경변수에서 다음 값을 로딩하며, 설정되지 않으면 에러를 반환합니다:
/// - CLIENT_ID (필수)
/// - CLIENT_SECRET (필수)
/// - REDIRECT_URL (선택, 기본값: http://127.0.0.1:3000/auth/authorized)
/// - AUTH_URL (선택, 기본값: Discord 권한 부여 URL)
/// - TOKEN_URL (선택, 기본값: Discord 토큰 교환 URL)
fn oauth_client() -> Result<BasicClient, AppError> {
    // let client_id = env::var("CLIENT_ID").context("Missing CLIENT_ID!")?;
    // let client_secret = env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET!")?;
    let client_id = "1363140223600562327".to_string(); // 실무에선 환경변수로..
    let client_secret = "yQxHBQB9twUx4DIXXUZ4fO2fF_TOSUya".to_string(); // 실무에선 환경변수로..

    let redirect_url = env::var("REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/authorized".to_string());

    let auth_url = env::var("AUTH_URL").unwrap_or_else(|_| {
        "https://discord.com/api/oauth2/authorize?response_type=code".to_string()
    });

    let token_url = env::var("TOKEN_URL")
        .unwrap_or_else(|_| "https://discord.com/api/oauth2/token".to_string());

    Ok(BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url).context("failed to create new authorization server URL")?,
        Some(TokenUrl::new(token_url).context("failed to create new token endpoint URL")?),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_url).context("failed to create new redirection URL")?,
    ))
}

/// ✅ Discord 유저 정보 구조체
/// - Discord API(`/users/@me`)로부터 응답받는 사용자 객체 형식
/// - 로그인 후 이 정보를 세션에 저장하고, 보호된 라우트에서 사용
#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: String,             // Discord 사용자 ID
    avatar: Option<String>, // 아바타 URL (없을 수 있음)
    username: String,       // 유저 이름
    discriminator: String,  // #0000 형식의 식별자
}

/// ✅ 인덱스 라우트 핸들러: `/`
/// - 로그인 여부에 따라 메시지를 다르게 출력
/// - 로그인된 경우 사용자 이름 출력, 아니면 로그인 안내 메시지
async fn index(user: Option<User>) -> impl IntoResponse {
    match user {
        Some(u) => format!(
            "Hey {}! You're logged in!\nYou may now access `/protected`.\nLog out with `/logout`.",
            u.username
        ),
        None => "You're not logged in.\nVisit `/auth/discord` to do so.".to_string(),
    }
}

/// ✅ 로그인 요청 처리 핸들러: `/auth/discord`
/// - 사용자 브라우저를 Discord 로그인 페이지로 리다이렉트
/// - CSRF 토큰을 생성하여 세션에 저장하고, 세션 쿠키를 응답에 포함
/// - 추후 `/auth/authorized`에서 CSRF 검증에 사용됨
async fn discord_auth(
    State(client): State<BasicClient>,
    State(store): State<MemoryStore>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Discord OAuth 인증 URL 생성 및 CSRF 토큰 획득
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("identify".to_string()))
        .url();

    // 2. 새로운 세션 생성 후, CSRF 토큰을 세션에 저장
    let mut session = Session::new();
    session
        .insert(CSRF_TOKEN, &csrf_token)
        .context("failed in inserting CSRF token into session")?;

    // 3. 세션 저장소에 저장하고, 세션 쿠키 값을 받아옴
    let cookie = store
        .store_session(session)
        .await
        .context("failed to store CSRF token session")?
        .context("unexpected error retrieving CSRF cookie value")?;

    // 4. 쿠키를 응답 헤더에 설정 (보안 설정 포함)
    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    // 5. Discord OAuth URL로 리다이렉트 응답 반환
    Ok((headers, Redirect::to(auth_url.as_ref())))
}

/// ✅ 보호된 라우트 핸들러: `/protected`
/// - 로그인된 사용자만 접근할 수 있음
/// - `User` 추출기가 세션에서 사용자 정보를 가져옴
/// - 인증되지 않은 사용자는 `/auth/discord`로 리다이렉트됨
async fn protected(user: User) -> impl IntoResponse {
    format!("Welcome to the protected area :)\nHere's your info:\n{user:?}")
}

/// ✅ 로그아웃 핸들러: `/logout`
/// - 쿠키에서 세션 ID를 가져와 해당 세션을 파기합니다.
/// - 세션이 없다면 그냥 `/` 경로로 리다이렉트만 수행
/// - 로그아웃 후 사용자 인증 정보는 서버에서 삭제됨
async fn logout(
    State(store): State<MemoryStore>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    // 1. 쿠키에서 세션 ID 추출
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?;

    // 2. 세션 로딩 시도
    let session = match store
        .load_session(cookie.to_string())
        .await
        .context("failed to load session")?
    {
        Some(s) => s,
        // 세션이 없으면 그냥 홈으로 리다이렉트
        None => return Ok(Redirect::to("/")),
    };

    // 3. 세션 파기 (MemoryStore 내 데이터 삭제)
    store
        .destroy_session(session)
        .await
        .context("failed to destroy session")?;

    // 4. 홈으로 리다이렉트
    Ok(Redirect::to("/"))
}

/// Discord OAuth2 서버로부터 전달받는 쿼리 파라미터 구조체
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AuthRequest {
    code: String,  // Authorization Code
    state: String, // CSRF 토큰 값
}

/// ✅ CSRF 토큰 검증 로직 (내부 사용)
/// - 요청에 포함된 `state` 값과, 세션에 저장된 `csrf_token` 값이 일치하는지 확인
/// - 검증 실패 시 인증 오류 반환
async fn csrf_token_validation_workflow(
    auth_request: &AuthRequest,
    cookies: &headers::Cookie,
    store: &MemoryStore,
) -> Result<(), AppError> {
    // 1. 쿠키에서 세션 ID 추출
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?
        .to_string();

    // 2. 세션 로딩
    let session = match store
        .load_session(cookie)
        .await
        .context("failed to load session")?
    {
        Some(session) => session,
        None => return Err(anyhow!("Session not found").into()),
    };

    // 3. 세션에서 저장된 CSRF 토큰 값 추출
    let stored_csrf_token = session
        .get::<CsrfToken>(CSRF_TOKEN)
        .context("CSRF token not found in session")?
        .to_owned();

    // 4. 세션 제거 (CSRF 토큰은 일회성이므로)
    store
        .destroy_session(session)
        .await
        .context("Failed to destroy old session")?;

    // 5. 세션 값과 전달된 state 값이 일치하는지 확인
    if *stored_csrf_token.secret() != auth_request.state {
        return Err(anyhow!("CSRF token mismatch").into());
    }

    Ok(())
}

/// ✅ OAuth 인증 완료 후 콜백 처리 핸들러: `/auth/authorized`
/// - Discord 인증 서버에서 Authorization Code와 함께 state(csrf_token) 전달됨
/// - 세션에서 저장된 CSRF 토큰과 비교하여 유효성 확인
/// - 토큰 교환 후, 사용자 정보를 요청하여 세션에 저장
/// - 세션 쿠키를 다시 발급하여 클라이언트에 전달하고 루트로 리다이렉트
async fn login_authorized(
    Query(query): Query<AuthRequest>,
    State(store): State<MemoryStore>,
    State(oauth_client): State<BasicClient>,
    TypedHeader(cookies): TypedHeader<headers::Cookie>,
) -> Result<impl IntoResponse, AppError> {
    // 1. CSRF 토큰 유효성 검증
    csrf_token_validation_workflow(&query, &cookies, &store).await?;

    // 2. Authorization Code → Access Token 교환
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .context("failed in sending request request to authorization server")?;

    // 3. Discord API로 사용자 정보 요청
    let client = reqwest::Client::new();
    let user_data: User = client
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .context("failed in sending request to target Url")?
        .json::<User>()
        .await
        .context("failed to deserialize response as JSON")?;

    // 4. 사용자 정보를 세션에 저장
    let mut session = Session::new();
    session
        .insert("user", &user_data)
        .context("failed in inserting serialized value into session")?;

    // 5. 세션 저장 및 쿠키 발급
    let cookie = store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("unexpected error retrieving cookie value")?;

    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );

    // 6. 루트 경로로 리다이렉트
    Ok((headers, Redirect::to("/")))
}

/// 인증 실패 시 로그인 페이지로 리다이렉트하는 타입
struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/discord").into_response()
    }
}

/// ✅ 커스텀 요청 추출기: `impl FromRequestParts for User`
/// - 세션 쿠키에서 사용자 정보를 꺼내 `User`로 복원
/// - 세션이 없거나 사용자 정보가 없으면 `/auth/discord`로 리다이렉트
impl<S> FromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // MemoryStore 추출
        let store = MemoryStore::from_ref(state);

        // 쿠키 파싱
        let cookies = parts
            .extract::<TypedHeader<headers::Cookie>>()
            .await
            .map_err(|e| match *e.name() {
                header::COOKIE => match e.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("unexpected error getting Cookie header(s): {e}"),
                },
                _ => panic!("unexpected error getting cookies: {e}"),
            })?;

        // 세션 ID 추출
        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;

        // 세션 로딩
        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;

        // 세션에서 사용자 정보 꺼내기
        let user = session.get::<User>("user").ok_or(AuthRedirect)?;

        Ok(user)
    }
}

/// ✅ Optional 추출기 구현: 로그인 상태가 아니어도 허용됨
/// - 존재하면 Some(User), 없으면 None
impl<S> OptionalFromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <User as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(res) => Ok(Some(res)),
            Err(AuthRedirect) => Ok(None),
        }
    }
}

/// ✅ 에러 핸들러: AppError 타입 정의 및 변환 구현
/// - 내부적으로 anyhow::Error를 감싸며 모든 에러를 일관되게 처리 가능하게 함
/// - axum에서 AppError가 발생하면 HTTP 500 상태 코드와 간단한 메시지를 반환
/// - 디버깅을 위해 로그 출력 포함
#[derive(Debug)]
struct AppError(anyhow::Error);

/// AppError를 axum HTTP 응답으로 변환
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // 에러를 로그로 출력
        tracing::error!("Application error: {:#}", self.0);

        // HTTP 500 응답 반환
        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

/// 모든 anyhow 호환 에러를 AppError로 자동 변환 가능하게 함
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// ✅ 마무리 요약:
// - 이 예제는 Discord OAuth 인증 흐름을 Axum + async_session 기반으로 구현한 전체적인 인증 플로우를 담고 있음
// - 로그인, 토큰 교환, 세션 기반 상태 유지, 보호된 라우트, 로그아웃, CSRF 보호 등 실무 구성의 좋은 참고 예시
// - MemoryStore는 데모 용도이며, Redis, DynamoDB 등으로 대체 필요
// - 실제 배포 시 HTTPS 적용 및 Secure 쿠키, CSRF 강화, state 무결성 검사 추가 고려

// [ 사용자 행동 ] → [ 인증 요청 생성 ] → [ CSRF 보호 ] → [ Authorization Code 교환 ]
// → [ Access Token 획득 ] → [ 사용자 정보 API 호출 ] → [ 세션 생성 & 쿠키 설정 ]
// → [ 인증된 상태 유지 ] → [ 보호 라우트 접근 허용 ]
