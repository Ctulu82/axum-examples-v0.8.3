//! Axum 서버에서 정상적인 서버 종료(Graceful Shutdown) 를 구현.

//! Graceful shutdown이란?
//! > 종료 신호(Ctrl+C 등)를 받으면,
//! > 진행 중인 요청은 마무리하고, 새 요청은 받지 않으며,
//! > 일정 시간 후에 서버를 깨끗하게 종료하는 패턴.
//! 이 기능은 실서비스에서 배포, 재시작, 롤링 업데이트 시 매우 중요!

// 종료 테스트용으로 5초 지연을 만들기 위해 필요
use std::time::Duration;

use axum::{routing::get, Router};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::time::sleep;
use tower_http::timeout::TimeoutLayer; // 요청 타임아웃 설정
use tower_http::trace::TraceLayer; // 요청 로깅
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 🚀 메인 함수

#[tokio::main]
async fn main() {
    // 로그 시스템 설정
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer().without_time()) // 타임스탬프 없는 로그
        .init();

    // 라우터 생성
    let app = Router::new()
        // 5초 지연 응답 (5초 뒤에 완료되는 요청)
        .route("/slow", get(|| sleep(Duration::from_secs(5))))
        // 절대 응답이 없는 요청 (무한 대기, 즉 절대 완료되지 않는 테스트용 요청)
        .route("/forever", get(std::future::pending::<()>))
        // 미들웨어 추가: 로그 + 타임아웃
        .layer((
            TraceLayer::new_for_http(),                 // HTTP 요청 추적 로그
            TimeoutLayer::new(Duration::from_secs(10)), // 요청당 최대 10초 허용
        ));

    // TCP 리스너 바인딩 (포트 3000)
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    // Graceful shutdown 설정
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal()) // 종료 시그널 대기
        .await
        .unwrap();
}

// 🧠 종료 신호 처리 함수

// 종료 신호를 대기하는 async 함수
async fn shutdown_signal() {
    // Ctrl+C (SIGINT)
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    // UNIX 환경일 경우: SIGTERM (kill 명령어 등)
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    // Windows 등의 non-UNIX 환경에선 대기만
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // 둘 중 먼저 오는 시그널을 기다림
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

// 🧪 테스트 방법

// 1. 서버 실행
//
// 2. 요청 보내기
// curl http://localhost:3000/slow
//
// 3. 그 상태에서 Ctrl+C 누르기
// 요청은 계속 진행되고, 5초 뒤에 완료됩니다. ✅
// forever 경로는 10초 타임아웃 이후 강제 종료됩니다. ⏳
