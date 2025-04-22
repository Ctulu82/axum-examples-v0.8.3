# auto-reload

이 예제는 소스 코드가 변경될 때마다 앱이 다시 컴파일되고 재시작되도록 axum 서비스의 개발 환경을 설정하는 방법을 보여줍니다.
`listenfd`를 사용하여 이전 버전의 앱에서 새로 컴파일된 버전으로 연결을 마이그레이션할 수 있습니다.

## Setup

```sh
cargo install cargo-watch systemfd
```

## Running

```sh
systemfd --no-pid -s http::3000 -- cargo watch -x run
```
