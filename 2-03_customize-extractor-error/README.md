이 예제는 기존 추출기(Extractor) 에 대해 커스텀 거부(Custom Rejection) 를 만드는 3가지 방법을 소개합니다.

- [`with_rejection`](src/with_rejection.rs): `axum_extra::extract::WithRejection` 을 사용하여 하나의 거부를 다른 거부로 변환하는 방법
- [`derive_from_request`](src/derive_from_request.rs): `axum::extract::FromRequest` 의 derive 매크로를 이용해 기존 추출기를 감싸고 거부 처리를 커스터마이즈하는 방법
- [`custom_extractor`](src/custom_extractor.rs): 기존 추출기를 감싸면서 `FromRequest` 를 수동으로 구현하는 방법

실행 방법:

```sh
cargo run -p example-customize-extractor-error
```
