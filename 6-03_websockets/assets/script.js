// 🔌 서버와 WebSocket 연결을 시작합니다.
const socket = new WebSocket("ws://localhost:3000/ws");

// ✅ 연결이 성공적으로 열렸을 때 호출됩니다.
socket.addEventListener("open", function (event) {
  // 서버에 간단한 텍스트 메시지 전송
  socket.send("Hello Server!");
});

// 📩 서버로부터 메시지를 수신할 때 호출됩니다.
socket.addEventListener("message", function (event) {
  console.log("Message from server ", event.data);
});

// ⏱️ 1초 후 JSON 데이터를 Blob으로 만들어 전송합니다.
setTimeout(() => {
  // 보낼 객체
  const obj = { hello: "world" };

  // JSON 객체를 Blob으로 변환하여 전송
  const blob = new Blob([JSON.stringify(obj, null, 2)], {
    type: "application/json",
  });

  console.log("Sending blob over websocket");
  socket.send(blob); // WebSocket으로 binary 데이터 전송
}, 1000);

// ⏱️ 3초 후 종료 메시지를 보내고 연결을 닫습니다.
setTimeout(() => {
  socket.send("About done here...");

  console.log("Sending close over websocket");
  socket.close(3000, "Crash and Burn!");
  // 종료 코드: 3000 (사용자 정의)
  // 종료 이유: "Crash and Burn!" → 서버에서 로그로 확인 가능
}, 3000);
