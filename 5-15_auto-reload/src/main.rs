//! 서버를 재시작하지 않아도 소스 코드 변경 시 새로운 바이너리로 자동 전환되는 기능을 구현하는 예제.
//! 핵심은 listenfd 를 이용하여 기존 소켓을 새로운 프로세스에서 재사용하는 것.
//! ```not_rust
//! cargo run -p auto-reload
//! ```

use axum::{response::Html, routing::get, Router};
use listenfd::ListenFd;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // axum 라우터 구성
    let app = Router::new().route("/", get(handler));

    // ListenFd 객체 생성: systemd, cargo-watch 등의 socket 전파 기능 활용
    let mut listenfd = ListenFd::from_env();

    // 기존 리스너가 존재하는지 확인
    let listener = match listenfd.take_tcp_listener(0).unwrap() {
        // if we are given a tcp listener on listen fd 0, we use that one
        // 기존 프로세스로부터 전달된 소켓이 있다면 사용
        Some(listener) => {
            // 비동기 처리를 위해 논블로킹으로 설정
            listener.set_nonblocking(true).unwrap();
            // std::net::TcpListener → tokio::net::TcpListener 로 변환
            TcpListener::from_std(listener).unwrap()
        }
        // otherwise fall back to local listening
        // 그렇지 않으면 기본적으로 새 리스너 생성
        None => TcpListener::bind("127.0.0.1:3000").await.unwrap(),
    };

    // 서버 시작
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// GET / 요청 처리 핸들러
async fn handler() -> Html<&'static str> {
    // println!("fixed something.."); // test 시 주석 해제!
    Html("<h1>Hello, World!</h1>")
}

// 🔁 auto-reload 작동 방식 설명
//
// listenfd: 시스템이 전달한 소켓 FD(파일 디스크립터)를 받아서 재사용.
// cargo watch: 코드 변경 감지 후 run 으로 서버 재시작.
// TcpListener::from_std: 기존 소켓을 비동기로 변환하여 새로운 서버에 이식.

// ⸻

// 🛠️ 개발 시 워크플로우
// 	1.	아래처럼 cargo-watch 설치 (처음 1회)
//      cargo install cargo-watch
//
// 	2.	예제 실행
//      cargo watch -x run
//
//  3.	src/main.rs를 수정하면:
// 	•	기존 소켓은 종료되지 않고
// 	•	새로운 프로세스에서 동일 포트로 이어받아 서버가 재시작됨
// 	•	브라우저나 curl 요청이 끊기지 않고 동작

// ⸻

// ✅ 왜 유용한가?
// 	•	개발 중 서버를 종료하고 재시작하는 번거로움 제거
// 	•	소켓 바인딩 충돌 없음 (항상 포트 3000에서 수신 가능)
// 	•	systemd, launchd 등의 init 시스템과도 호환 가능

// ⸻

// ❗ 참고
// 	•	listenfd는 프로덕션 용도가 아닌 개발 환경용 도구
// 	•	axum-server의 bind_with_graceful_shutdown() 등과 함께 쓰면 더 좋음
// 	•	단순한 로컬 개발 워크플로우에서 유용
