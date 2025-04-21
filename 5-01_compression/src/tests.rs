//! compression 예제 - tests.rs 한글 주석 포함
//!
//! 요청 바디 압축/해제 및 응답 압축 동작을 검증하는 통합 테스트입니다.

use assert_json_diff::assert_json_eq;
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use brotli::enc::BrotliEncoderParams;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use http::{header, StatusCode};
use serde_json::{json, Value};
use std::io::{Read, Write};
use tower::ServiceExt;

use super::*;

/// ✅ 압축되지 않은 JSON 요청 테스트
#[tokio::test]
async fn handle_uncompressed_request_bodies() {
    let body = json();

    let compressed_request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(json_body(&body))
        .unwrap();

    let response = app().oneshot(compressed_request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

/// ✅ gzip 압축 요청 테스트
#[tokio::test]
async fn decompress_gzip_request_bodies() {
    let body = compress_gzip(&json());

    let compressed_request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "gzip")
        .body(Body::from(body))
        .unwrap();

    let response = app().oneshot(compressed_request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

/// ✅ brotli 압축 요청 테스트
#[tokio::test]
async fn decompress_br_request_bodies() {
    let body = compress_br(&json());

    let compressed_request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "br")
        .body(Body::from(body))
        .unwrap();

    let response = app().oneshot(compressed_request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

/// ✅ zstd 압축 요청 테스트
#[tokio::test]
async fn decompress_zstd_request_bodies() {
    let body = compress_zstd(&json());

    let compressed_request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_ENCODING, "zstd")
        .body(Body::from(body))
        .unwrap();

    let response = app().oneshot(compressed_request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

/// ✅ 응답 압축 없이 동작 확인
#[tokio::test]
async fn do_not_compress_response_bodies() {
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(json_body(&json()))
        .unwrap();

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_json_eq!(json_from_response(response).await, json());
}

/// ✅ gzip 응답 압축 테스트
#[tokio::test]
async fn compress_response_bodies_with_gzip() {
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "gzip")
        .body(json_body(&json()))
        .unwrap();

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let mut decoder = GzDecoder::new(response_body.as_ref());
    let mut decompress_body = String::new();
    decoder.read_to_string(&mut decompress_body).unwrap();
    assert_json_eq!(
        serde_json::from_str::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
}

/// ✅ brotli 응답 압축 테스트
#[tokio::test]
async fn compress_response_bodies_with_br() {
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "br")
        .body(json_body(&json()))
        .unwrap();

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let mut decompress_body = Vec::new();
    brotli::BrotliDecompress(&mut response_body.as_ref(), &mut decompress_body).unwrap();
    assert_json_eq!(
        serde_json::from_slice::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
}

/// ✅ zstd 응답 압축 테스트
#[tokio::test]
async fn compress_response_bodies_with_zstd() {
    let request = http::Request::post("/")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT_ENCODING, "zstd")
        .body(json_body(&json()))
        .unwrap();

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = byte_from_response(response).await;
    let decompress_body = zstd::stream::decode_all(std::io::Cursor::new(response_body)).unwrap();
    assert_json_eq!(
        serde_json::from_slice::<serde_json::Value>(&decompress_body).unwrap(),
        json()
    );
}

/// ✅ 테스트에 사용될 샘플 JSON 객체 생성
fn json() -> Value {
    json!({
      "name": "foo",
      "mainProduct": {
        "typeId": "product",
        "id": "p1"
      },
    })
}

/// JSON 객체를 요청 바디로 변환
fn json_body(input: &Value) -> Body {
    Body::from(serde_json::to_vec(&input).unwrap())
}

/// 응답 바디에서 JSON 디코딩
async fn json_from_response(response: Response) -> Value {
    let body = byte_from_response(response).await;
    body_as_json(body)
}

/// 응답 바디에서 원시 바이트 추출
async fn byte_from_response(response: Response) -> Bytes {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
}

/// 바이트 → JSON 역직렬화
fn body_as_json(body: Bytes) -> Value {
    serde_json::from_slice(body.as_ref()).unwrap()
}

/// gzip 압축 함수
fn compress_gzip(json: &Value) -> Vec<u8> {
    let request_body = serde_json::to_vec(&json).unwrap();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&request_body).unwrap();
    encoder.finish().unwrap()
}

/// brotli 압축 함수
fn compress_br(json: &Value) -> Vec<u8> {
    let request_body = serde_json::to_vec(&json).unwrap();
    let mut result = Vec::new();
    let params = BrotliEncoderParams::default();
    let _ = brotli::enc::BrotliCompress(&mut &request_body[..], &mut result, &params).unwrap();
    result
}

/// zstd 압축 함수
fn compress_zstd(json: &Value) -> Vec<u8> {
    let request_body = serde_json::to_vec(&json).unwrap();
    zstd::stream::encode_all(std::io::Cursor::new(request_body), 4).unwrap()
}
