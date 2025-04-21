# 📦 compression

이 예제는 다음과 같은 기능을 보여줍니다:
- 클라이언트 요청 바디가 압축되어 있을 경우, 자동으로 압축 해제
- 서버 응답 바디를 클라이언트의 `accept` 헤더에 따라 압축해서 전달
---

## 🏃 실행 방법

```
cargo run -p example-compression
```
---

## 📤 압축된 요청 보내기

```
curl -v -g 'http://localhost:3000/' \
    -H "Content-Type: application/json" \
    -H "Content-Encoding: gzip" \
    --compressed \
    --data-binary @data/products.json.gz
```

- Postman으로 테스트가 불가한 것으로 보이며, 프로젝트의 root경로에서 터미널로 실행!
- 요청에 `Content-Encoding: gzip` 헤더가 포함되어 있고,
- 응답에도 `content-encoding: gzip` 헤더가 포함되어 있는 것을 확인할 수 있습니다.
---

## 📥 압축되지 않은 요청 보내기

```
curl -v -g 'http://localhost:3000/' \
    -H "Content-Type: application/json" \
    --compressed \
    --data-binary @data/products.json
```

- Postman, terminal 모두 테스트 가능!
- 이 경우 요청은 일반 JSON이며, 클라이언트가 Accept-Encoding을 통해 압축 응답을 요청할 수 있습니다.
---
