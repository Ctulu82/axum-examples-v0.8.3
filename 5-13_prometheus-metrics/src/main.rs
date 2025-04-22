//!
//! Prometheus (모니터링 및 메트릭 수집 툴)을 활용하여 Axum 서버의 요청 수, 응답 시간 등의 지표(metrics) 를 기록하고,
//! 이를 /metrics 엔드포인트로 노출하는 구조를 보여주는 실전 지향 예제.
//!
//! tower-http에서 공식 metrics 미들웨어가 제공되기 전까지
//! Prometheus를 활용하여 직접 메트릭을 수집하는 예제임.
//!

use axum::{
    extract::{MatchedPath, Request},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    Router,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::{
    future::ready,
    time::{Duration, Instant},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================
// /metrics 엔드포인트 구성
// ============================

fn metrics_app() -> Router {
    let recorder_handle = setup_metrics_recorder();

    // GET /metrics 요청 시 Prometheus 포맷으로 메트릭 렌더링
    Router::new().route("/metrics", get(move || ready(recorder_handle.render())))
}

// ============================
// 실제 서비스용 라우터 구성
// ============================

fn main_app() -> Router {
    Router::new()
        .route("/fast", get(|| async {})) // 빠른 응답
        .route(
            "/slow", // 느린 응답 (1초 대기)
            get(|| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }),
        )
        // 모든 요청에 대해 메트릭 추적 미들웨어 적용
        .route_layer(middleware::from_fn(track_metrics))
}

// ============================
// 첫 번째 서버: 메인 서비스 서버 (포트 3000)
// ============================

async fn start_main_server() {
    let app = main_app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// ============================
// 두 번째 서버: /metrics 전용 (포트 3001)
// ============================

async fn start_metrics_server() {
    let app = metrics_app();

    // 실무에서는 /metrics 를 외부에 노출하지 않도록 별도 포트로 구성함
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

// ============================
// 메인 진입점: 두 서버를 병렬로 실행
// ============================

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // tower_http 로그까지 포함하여 디버깅 가능
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 두 개의 서버를 병렬로 실행 (main + metrics)
    let (_main_server, _metrics_server) = tokio::join!(start_main_server(), start_metrics_server());
}

// ============================
// Prometheus 레코더 설정
// ============================

fn setup_metrics_recorder() -> PrometheusHandle {
    // 응답 시간 측정을 위한 버킷 구간 (초 단위)
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    // http_requests_duration_seconds 메트릭에 대한 히스토그램 버킷 구성
    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install_recorder() // 전역 레코더로 등록
        .unwrap()
}

// ============================
// 메트릭 추적 미들웨어
// ============================

async fn track_metrics(req: Request, next: Next) -> impl IntoResponse {
    // 시작 시간 기록
    let start = Instant::now();

    // 요청 경로 추출 (라우팅 매칭된 path 우선)
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };

    let method = req.method().clone();

    // 다음 미들웨어 또는 실제 핸들러 실행
    let response = next.run(req).await;

    // 요청 처리 시간 계산
    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // 메트릭 라벨 구성
    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    // 총 요청 수 증가
    metrics::counter!("http_requests_total", &labels).increment(1);

    // 요청 응답 시간 기록
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}

// 🙅🏽 Prometheus 설치는 필수는 아님.
// 예제에서 라우팅 요청(즉, HTTP 요청에 대한 메트릭)은 디스크나 DB에 저장되지 않음.
// 메모리(RAM) 에만 임시로 저장됨.

// 🔄 흐름 요약
//     [HTTP 요청]
//        ↓
//     [track_metrics() 미들웨어]
//        ↓
//     metrics::counter!(), metrics::histogram!()
//        ↓
//     [metrics_exporter_prometheus 내부의 RAM-based storage]
//        ↓
//     [GET /metrics 요청 → 저장된 메트릭을 Prometheus 형식으로 출력]

// 🧪 테스트 방법
//
// 1. /fast, /slow 엔드포인트에 curl 요청:
//    curl http://127.0.0.1:3000/fast
//    curl http://127.0.0.1:3000/slow
//
// 2. /metrics 확인 (다른 터미널에서):
//    curl http://127.0.0.1:3001/metrics

// ⸻

// 📊 Prometheus 툴과의 연동
// 	•	이 예제는 Prometheus 형식으로 메트릭을 제공합니다.
// 	•	실제 운영에서는:
// 	•	Prometheus 서버 설정에서 http://<your-host>:3001/metrics 를 scrape target으로 등록
// 	•	Grafana 같은 대시보드에서 시각화
