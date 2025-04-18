//! Example JWT authorization/authentication.
//! JWT 기반 인증 및 보호된 라우트 처리를 다루는 실전 웹 앱에 가까운 구조를 보여주는 예제.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, RequestPartsExt, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;
use std::sync::LazyLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🔐 JWT 키를 초기화하는 전역 정적 객체
/// 실행 중 최초 접근 시 환경변수 JWT_SECRET로부터 키를 읽어 Encoding/Decoding 키를 설정.
/// LazyLock은 처음 접근할 때만 초기화됨. (once_cell::sync::Lazy의 최신 버전 alias)
static KEYS: LazyLock<Keys> = LazyLock::new(|| {
    // let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let secret = "secret"; // $ JWT_SECRET=secret cargo run -p example-jwt 를 대체
    Keys::new(secret.as_bytes())
});

/// 🔧 main 함수
#[tokio::main]
async fn main() {
    // tracing_subscriber을 설정하여 로깅을 구성
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // API 라우터 구성
    let app = Router::new()
        .route("/protected", get(protected)) // JWT 인증이 필요한 라우트
        .route("/authorize", post(authorize)); // JWT 토큰을 발급받는 라우트

    // 서버를 127.0.0.1:3000 포트에 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

/// ✅ GET /protected: JWT를 헤더에 담아야 접근 가능한 보호된 API
/// •	즉, 유효한 JWT가 있을 경우에만 접근 가능.
/// •	이 함수에서 'Claims'가 파라미터로 직접 들어오는 점이 중요.
/// •	Axum은 요청에서 자동으로 JWT를 추출 → 디코딩 → 검증하여 Claims로 변경해줌.
/// •	이 처리를 가능하게 하는 것이 FromRequestParts의 구현.
async fn protected(claims: Claims) -> Result<String, AuthError> {
    // JWT 내부 클레임 정보를 포맷팅하여 응답합니다.
    Ok(format!(
        "Welcome to the protected area :)\nYour data:\n{claims}",
    ))
}

/// 🔓 POST /authorize: 사용자 자격증명(client_id, client_secret)을 받아 JWT를 발급
/// •	사용자 자격 정보를 받아 JWT를 생성해줌.
/// •	클라이언트가 보낸 client_id, client_secret이 “foo”, “bar”와 일치하면 JWT 토큰 발급
async fn authorize(Json(payload): Json<AuthPayload>) -> Result<Json<AuthBody>, AuthError> {
    // 클라이언트 ID 또는 시크릿이 비어있으면(자격증명 누락) 에러 반환
    if payload.client_id.is_empty() || payload.client_secret.is_empty() {
        return Err(AuthError::MissingCredentials);
    }

    // 고정된 사용자 인증 정보와 일치하지 않으면 인증 실패 처리
    // (실제 서비스에서는 DB 조회로 대체되어야 함!)
    if payload.client_id != "foo" || payload.client_secret != "bar" {
        return Err(AuthError::WrongCredentials);
    }

    // 토큰에 담을 사용자 정보 클레임 생성
    let claims = Claims {
        sub: "b@b.com".to_owned(),
        company: "ACME".to_owned(),
        exp: 2000000000, // 만료 시간 (UTC UNIX timestamp: 2033년)
    };

    // JWT 토큰 생성 (암호화 실패 시 에러 처리)
    let token = encode(&Header::default(), &claims, &KEYS.encoding)
        .map_err(|_| AuthError::TokenCreation)?;

    // JWT를 포함한 응답 본문 반환
    Ok(Json(AuthBody::new(token)))
}

/// Claims 구조체를 문자열로 포맷팅해주는 구현
impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Email: {}\nCompany: {}", self.sub, self.company)
    }
}

/// 응답용 JWT 토큰 본문을 생성하는 헬퍼 함수
impl AuthBody {
    fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}

/// 🔁 사용자 요청 헤더에서 JWT를 추출하고 검증하여 Claims로 변환하는 커스텀 추출기 구현
/// •	Authorization: Bearer <토큰> 형식의 헤더에서 JWT를 추출
/// •	jsonwebtoken::decode를 통해 Claims로 디코딩
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 헤더에서 Authorization: Bearer <token> 형식의 토큰 추출
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        // JWT를 디코딩하여 Claims 추출 (검증 실패 시 에러 반환)
        let token_data = decode::<Claims>(bearer.token(), &KEYS.decoding, &Validation::default())
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims)
    }
}

/// 인증 관련 에러를 HTTP 응답으로 변환하는 구현
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

/// 🧰 JWT 인코딩/디코딩 키를 보관하는 구조체

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

/// 🧾 JWT에 담기는 클레임 구조체 (사용자 정보 및 만료시간 포함)
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,     // 사용자 이메일 또는 ID
    company: String, // 부가 정보
    exp: usize,      // 만료 시간 (UTC timestamp)
}

/// JWT 토큰을 담아 클라이언트에 반환할 구조체
#[derive(Debug, Serialize)]
struct AuthBody {
    access_token: String,
    token_type: String,
}

/// 클라이언트 인증 요청 시 전달받는 JSON 구조체
#[derive(Debug, Deserialize)]
struct AuthPayload {
    client_id: String,
    client_secret: String,
}

/// 🧨 인증 관련 에러 종류 정의
#[derive(Debug)]
enum AuthError {
    WrongCredentials,   // 자격 정보 불일치
    MissingCredentials, // 자격 정보 누락
    TokenCreation,      // 토큰 생성 실패
    InvalidToken,       // 잘못된 토큰 또는 디코딩 실패
}

// 테스트 방법
//
// 인증 토큰 가져오기:
//  > POST http://localhost:3000/authorize
//  &
//  {"client_id":"foo","client_secret":"bar"}
//
// 유효한 JWT가 있을 경우에만 접근 가능한 API 사용하기 (성공):
//  > GET http://localhost:3000/protected
//  &
//  Authorization: Bearer ey...gM (POST 결과값 사용할 것!)
//
// 유효한 JWT가 있을 경우에만 접근 가능한 API 사용하기 (실패):
//  > GET http://localhost:3000/protected
//  &
//  Authorization: Bearer blahblahblah
