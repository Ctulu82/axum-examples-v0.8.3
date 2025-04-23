//! Axum + WebSocket 기반 채팅 서버 구현
//! 브라우저에서 JavaScript로 WebSocket을 연결하고,
//! 서버에서는 broadcast::channel을 사용해 모든 클라이언트 간 메시지를 공유하는 구조
//! Run with
//!
//! ```not_rust
//! cargo run -p example-chat
//! ```

use axum::{
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// ✅ 1. 상태 공유 구조체 정의

// Our shared state
struct AppState {
    // We require unique usernames. This tracks which usernames have been taken.
    // 중복 닉네임 방지를 위한 사용자 이름 저장소
    user_set: Mutex<HashSet<String>>,

    // Channel used to send messages to all connected clients.
    // 메시지를 모든 클라이언트에게 브로드캐스트하는 채널
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ✅ 2. main 함수 - 서버 및 상태 초기화

    // Set up application state for use with with_state().
    let user_set = Mutex::new(HashSet::new());
    // broadcast::channel은 하나가 메시지를 보내면 구독자 모두에게 전달
    let (tx, _rx) = broadcast::channel(100);

    // Arc: AppState를 여러 task 간 공유 가능하게 함
    let app_state = Arc::new(AppState { user_set, tx });

    let app = Router::new()
        .route("/", get(index))
        .route("/websocket", get(websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

/// ✅ 3. WebSocket 연결 핸들러
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // 클라이언트가 /websocket에 접속하면 on_upgrade를 통해 WebSocket으로 전환
    ws.on_upgrade(|socket| websocket(socket, state))
}

/// ✅ 4. 각 사용자의 WebSocket 처리

// This function deals with a single websocket connection, i.e., a single
// connected client / user, for which we will spawn two independent tasks (for
// receiving / sending chat messages).
async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // By splitting, we can send and receive at the same time.
    let (mut sender, mut receiver) = stream.split();

    // Username gets set in the receive loop, if it's valid.
    let mut username = String::new();

    // Loop until a text message is found.
    // 💬 사용자 이름 수신 및 중복 검사
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            // If username that is sent by client is not taken, fill username string.
            // 최초 메시지를 닉네임으로 간주
            check_username(&state, &mut username, name.as_str());

            // If not empty we want to quit the loop else we want to quit function.
            if !username.is_empty() {
                break;
            } else {
                // Only send our client that username is taken.
                // 중복이면 “Username already taken.” 메시지를 보내고 종료
                let _ = sender
                    .send(Message::Text(Utf8Bytes::from_static(
                        "Username already taken.",
                    )))
                    .await;

                return;
            }
        }
    }

    // We subscribe *before* sending the "joined" message, so that we will also
    // display it to our client.
    let mut rx = state.tx.subscribe();

    // Now send the "joined" message to all subscribers.
    // 사용자 입장을 브로드캐스트로 알림
    let msg = format!("{username} joined.");
    tracing::debug!("{msg}");
    let _ = state.tx.send(msg);

    //📡 메시지 송수신 Task 분리

    // Spawn the first task that will receive broadcast messages and send text
    // messages over the websocket to our client.
    // 브로드캐스트 수신해서 클라이언트에게 전송
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Clone things we want to pass (move) to the receiving task.
    let tx = state.tx.clone();
    let name = username.clone();

    // Spawn a task that takes messages from the websocket, prepends the user
    // name, and sends them to all broadcast subscribers.
    // 클라이언트로부터 수신한 메시지를 브로드캐스트로 전달
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Add username before message.
            let _ = tx.send(format!("{name}: {text}"));
        }
    });

    // If any one of the tasks run to completion, we abort the other.
    // 어느 한 쪽이 끊기면 다른 쪽 task도 종료 (정리 목적)
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    // Send "user left" message (similar to "joined" above).
    // 사용자 퇴장을 브로드캐스트로 알림
    let msg = format!("{username} left.");
    tracing::debug!("{msg}");
    let _ = state.tx.send(msg);

    // Remove username from map so new clients can take it again.
    // 닉네임은 다시 사용 가능하도록 user_set에서 제거
    state.user_set.lock().unwrap().remove(&username);
}

fn check_username(state: &AppState, string: &mut String, name: &str) {
    let mut user_set = state.user_set.lock().unwrap();

    if !user_set.contains(name) {
        user_set.insert(name.to_owned());

        string.push_str(name);
    }
}

///✅ 5. HTML 렌더링

// Include utf-8 file at **compile** time.
async fn index() -> Html<&'static str> {
    // 정적 파일을 컴파일 시점에 포함하여 / 경로에서 반환
    Html(std::include_str!("../chat.html"))
}

// ⸻

// 🧪 테스트 방법
// 	1.	브라우저에서 localhost:3000 접속
// 	2.	여러 탭에서 접속 후 닉네임 입력 → 채팅 메시지 입력
// 	3.	서버 로그에도 “joined”, “left” 로그 출력 확인

// ⸻

// ✅ 요약 흐름
// 브라우저 chat.html
//  └── WebSocket(ws://localhost:3000/websocket)
//       ├── 최초 메시지: 사용자 이름
//       ├── 이후 메시지: 채팅 텍스트
//       ├── 서버:
//       │   ├── 닉네임 확인 (중복 방지)
//       │   ├── 입장/퇴장 알림
//       │   ├── 수신 메시지 → broadcast 채널 전파
//       │   └── 전파 메시지 → 각 사용자에게 전달

// ⸻

// 이 예제는 Axum + WebSocket + 상태 공유의 정석적인 구조로, 실무에서도 쉽게 응용 가능합니다.
