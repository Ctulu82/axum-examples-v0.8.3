//! 실행 명령
//! ```bash
//! cargo run -p example-unix-domain-socket
//! ```
//! 서버와 클라이언트가 같은 프로그램 안에 구현되어 있어서 실행만 해도 동작 테스트가 자동 수행
//!
//! Axum 서버를 일반적인 TCP 포트가 아닌 Unix 도메인 소켓(UDS) 상에서 실행하는 방법을 보여줌
//!  > UDS: 동일한 호스트 내에서 실행 중인 두 프로세스가 파일 경로를 통해 직접 통신할 수 있도록 하는 로컬 IPC 방식
//!  > 실제 사용 사례: 컨테이너 내부 통신, nginx 프록시 백엔드 연결, 보안이 필요한 내부 API 연결 등
//!
//! 실행 개요
//! •	/tmp/axum/helloworld 경로에 Unix 소켓을 생성합니다.
//! •	서버는 해당 소켓에서 HTTP 요청을 수신합니다.
//! •	클라이언트는 동일한 소켓 경로를 통해 요청을 보내고 응답을 수신합니다.
//! •	이 모든 흐름은 하나의 Rust 프로그램 내에서 이루어지며, 실행 즉시 테스트도 함께 수행됩니다.
//!

#[cfg(unix)]
#[tokio::main]
async fn main() {
    unix::server().await;
}

#[cfg(not(unix))]
fn main() {
    println!("This example requires unix")
}

#[cfg(unix)]
mod unix {
    use axum::{
        body::Body,
        extract::connect_info::{self, ConnectInfo},
        http::{Method, Request, StatusCode},
        routing::get,
        serve::IncomingStream,
        Router,
    };
    use http_body_util::BodyExt;
    use hyper_util::rt::TokioIo;
    use std::{path::PathBuf, sync::Arc};
    use tokio::net::{unix::UCred, UnixListener, UnixStream};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    pub async fn server() {
        // 로그 초기화
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "debug".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        // 바인딩할 소켓 경로 설정
        let path = PathBuf::from("/tmp/axum/helloworld");

        // 기존 파일 제거 후 디렉터리 생성
        let _ = tokio::fs::remove_file(&path).await;
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();

        // Unix 도메인 소켓 리스너 생성
        let uds = UnixListener::bind(path.clone()).unwrap();

        // 서버 실행
        tokio::spawn(async move {
            let app = Router::new()
                .route("/", get(handler))
                .into_make_service_with_connect_info::<UdsConnectInfo>();

            axum::serve(uds, app).await.unwrap();
        });

        // 클라이언트 역할: UDS 소켓에 연결
        let stream = TokioIo::new(UnixStream::connect(path).await.unwrap());

        // Hyper 클라이언트: HTTP/1 핸드셰이크
        let (mut sender, conn) = hyper::client::conn::http1::handshake(stream).await.unwrap();

        // 커넥션 유지
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        // GET 요청 구성
        let request = Request::builder()
            .method(Method::GET)
            .uri("http://uri-doesnt-matter.com") // UDS라서 URI는 중요하지 않음
            .body(Body::empty())
            .unwrap();

        // 요청 전송 및 응답 받기
        let response = sender.send_request(request).await.unwrap();

        // 상태 코드 확인
        assert_eq!(response.status(), StatusCode::OK);

        // 본문 확인
        let body = response.collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    // GET / 요청 핸들러
    async fn handler(ConnectInfo(info): ConnectInfo<UdsConnectInfo>) -> &'static str {
        println!("new connection from `{:?}`", info); // peer UID 정보 출력
        "Hello, World!"
    }

    // UDS용 커넥션 정보 구조체
    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    struct UdsConnectInfo {
        peer_addr: Arc<tokio::net::unix::SocketAddr>, // 소켓 주소
        peer_cred: UCred,                             // 유닉스 사용자 인증 정보 (uid, gid, pid)
    }

    // 커넥션 정보 추출기 구현
    impl connect_info::Connected<IncomingStream<'_, UnixListener>> for UdsConnectInfo {
        fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
            let peer_addr = stream.io().peer_addr().unwrap(); // 클라이언트 주소
            let peer_cred = stream.io().peer_cred().unwrap(); // UCred 정보
            Self {
                peer_addr: Arc::new(peer_addr),
                peer_cred,
            }
        }
    }
}

// 흐름 요약
// 	1.	/tmp/axum/helloworld에 기존 소켓 파일이 존재한다면 삭제합니다.
// 	2.	디렉터리가 없다면 생성합니다.
// 	3.	UnixListener를 사용해 해당 소켓 경로에 서버를 바인딩합니다.
// 	4.	서버는 Axum Router를 구성하고 요청을 수신합니다.
// 	5.	별도로 클라이언트를 생성하여 UnixStream을 통해 해당 소켓에 연결합니다.
// 	6.	클라이언트는 HTTP 요청을 전송하고 응답을 검증합니다.
// 	7.	서버는 ConnectInfo를 통해 요청자의 UID, GID, PID 정보(UCred)를 출력합니다.

// ⸻

// 주요 기능 및 포인트
// 	•	UnixListener와 UnixStream은 TCP 포트를 사용하지 않고 파일 시스템을 통해 연결을 생성합니다.
// 	•	요청자는 peer_cred()를 통해 UID, GID 등의 인증 정보를 확인할 수 있습니다.
// 	•	Axum에서는 connect_info::Connected 트레잇을 구현하여 커넥션 정보를 추출할 수 있습니다.
// 	•	URI는 네트워크 주소가 아니기 때문에 실제 URL은 중요하지 않으며,
//      형식상 "http://uri-doesnt-matter.com"과 같은 더미 URI를 사용합니다.
// 	•	클라이언트 요청은 hyper::client::conn::http1::handshake()를 통해 설정되고,
//      응답 검증까지 수행되며 전체 로직이 자동 테스트처럼 실행됩니다.

// ⸻

// 실무 활용 예시
// 	•	컨테이너 내부에서 동작하는 백엔드 서비스와의 보안 통신에 사용될 수 있습니다.
// 	•	nginx와 연동하여 proxy_pass http://unix:/tmp/axum/helloworld: 방식으로 사용 가능합니다.
// 	•	시스템 수준에서 보안이 필요한 API 서버를 외부에 노출하지 않고 내부 서비스 간 통신에 활용할 수 있습니다.

// ⸻

// 응용 아이디어
// 	•	nginx 설정과 연계하여 프런트 → 백엔드 간 UDS 경로 기반 프록시 구성
// 	•	UDS 기반의 gRPC 서버 또는 파일기반 IPC 구조
// 	•	systemd 서비스와 연동하여 /run/app.sock 같은 경로를 사용하는 보안 강화 구성
