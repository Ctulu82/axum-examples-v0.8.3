//! 이 예제는 Axum 서버에서 의존성 주입(DI)을 실현하는 두 가지 방법을 보여줍니다:
//!
//! 1. trait object (`Arc<dyn UserRepo>`) 방식
//! 2. generic 타입 파라미터 (`T: UserRepo`) 방식
//!

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State}, // Path: 경로 변수 추출, State: 앱 상태 주입
    http::StatusCode,
    routing::{get, post},
    Json,
    Router,
};

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid; // 사용자 식별용 UUID

/// 🧭 메인 함수

#[tokio::main]
async fn main() {
    // 로그 시스템 초기화
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // In-memory user repo 생성 (공통)
    let user_repo = InMemoryUserRepo::default();

    // We generally have two ways to inject dependencies:
    //
    // 1. Using trait objects (`dyn SomeTrait`)
    //     - Pros
    //         - Likely leads to simpler code due to fewer type parameters.
    //     - Cons
    //         - Less flexible because we can only use object safe traits
    //         - Small amount of additional runtime overhead due to dynamic dispatch.
    //           This is likely to be negligible.
    // 2. Using generics (`T where T: SomeTrait`)
    //     - Pros
    //         - More flexible since all traits can be used.
    //         - No runtime overhead.
    //     - Cons:
    //         - Additional type parameters and trait bounds can lead to more complex code and
    //           boilerplate.
    //
    // Using trait objects is recommended unless you really need generics.

    // 방식 1. Trait Object 기반 DI (Arc<dyn Trait>)
    let using_dyn = Router::new()
        .route("/users/{id}", get(get_user_dyn)) // GET /dyn/users/{id}
        .route("/users", post(create_user_dyn)) // POST /dyn/users
        .with_state(AppStateDyn {
            user_repo: Arc::new(user_repo.clone()), // Arc로 감싼 dyn UserRepo
        });

    // 방식 2. Generic 기반 DI (T: Trait)
    let using_generic = Router::new()
        .route("/users/{id}", get(get_user_generic::<InMemoryUserRepo>))
        .route("/users", post(create_user_generic::<InMemoryUserRepo>))
        .with_state(AppStateGeneric { user_repo }); // 그대로 주입

    // `/dyn`과 `/generic` 경로를 각각 서브라우트로 묶음
    let app = Router::new()
        .nest("/dyn", using_dyn)
        .nest("/generic", using_generic);

    // 3000번 포트로 서버 실행
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// 📦 상태 구조체 정의

// dyn 방식: trait object를 Arc로 감싸서 보관
#[derive(Clone)]
struct AppStateDyn {
    user_repo: Arc<dyn UserRepo>,
}

// generic 방식: 타입 파라미터로 유연하게 보관
#[derive(Clone)]
struct AppStateGeneric<T> {
    user_repo: T,
}

/// 🧍 사용자 모델 및 입력 파라미터

#[derive(Debug, Serialize, Clone)]
struct User {
    id: Uuid,
    name: String,
}

#[derive(Deserialize)]
struct UserParams {
    name: String,
}

/// ✏️ 핸들러 함수 (trait object 기반)

// POST /dyn/users
async fn create_user_dyn(
    State(state): State<AppStateDyn>,
    Json(params): Json<UserParams>,
) -> Json<User> {
    let user = User {
        id: Uuid::new_v4(),
        name: params.name,
    };

    state.user_repo.save_user(&user);
    Json(user)
}

// GET /dyn/users/{id}
async fn get_user_dyn(
    State(state): State<AppStateDyn>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, StatusCode> {
    match state.user_repo.get_user(id) {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// ✏️ 핸들러 함수 (generic 기반)

// POST /generic/users
async fn create_user_generic<T>(
    State(state): State<AppStateGeneric<T>>,
    Json(params): Json<UserParams>,
) -> Json<User>
where
    T: UserRepo,
{
    let user = User {
        id: Uuid::new_v4(),
        name: params.name,
    };

    state.user_repo.save_user(&user);
    Json(user)
}

// GET /generic/users/{id}
async fn get_user_generic<T>(
    State(state): State<AppStateGeneric<T>>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, StatusCode>
where
    T: UserRepo,
{
    match state.user_repo.get_user(id) {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// 🧩 DI 대상이 될 Trait 및 구현체

// 저장소 Trait (인터페이스 개념)
trait UserRepo: Send + Sync {
    fn get_user(&self, id: Uuid) -> Option<User>;

    fn save_user(&self, user: &User);
}

/// 🧠 메모리 기반 저장소 구현

#[derive(Debug, Clone, Default)]
struct InMemoryUserRepo {
    map: Arc<Mutex<HashMap<Uuid, User>>>,
}

impl UserRepo for InMemoryUserRepo {
    fn get_user(&self, id: Uuid) -> Option<User> {
        self.map.lock().unwrap().get(&id).cloned()
    }

    fn save_user(&self, user: &User) {
        self.map.lock().unwrap().insert(user.id, user.clone());
    }
}

// ✅ 요청 예시
// 1. 사용자 생성
// curl -X POST http://localhost:3000/dyn/users \
//      -H "Content-Type: application/json" \
//      -d '{"name": "Alice"}'
// 2. 사용자 조회 (UUID는 위 결과에서 가져오기)
// curl http://localhost:3000/dyn/users/<uuid>
// ! 또는 ../generic/users 로 제너릭 DI 엔드포인트 테스트.

// ✅ 엔드포인트 요약
// dyn
// - 사용자 생성: POST /dyn/users
// - 사용자 조회: GET /dyn/users/{id}
// generic
// - 사용자 생성: POST /generic/users
// - 사용자 조회: GET /generic/users/{id}

// 🔍 두 DI 방식 비교
// Trait Object (dyn)
// - 유연성:	적당히 유연, 대부분 사용 가능
// - 성능:	약간의 런타임 오버헤드 있음
// - 제약:	object safe 트레잇만 사용 가능
// - 실무 적용:	빠른 개발, 인터페이스만 보면 충분
// Generic (T)
// - 유연성:	컴파일 시간에 모든 타입 고정
// - 성능:	고성능 (zero cost abstraction)
// - 제약:	어떤 트레잇이든 사용 가능
// - 실무 적용:	성능이 중요한 경우 또는 단일 구현일 경우 좋음
