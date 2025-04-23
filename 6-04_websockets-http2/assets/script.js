// WebSocket 연결을 HTTPS 기반 wss (WebSocket Secure)로 시작
const socket = new WebSocket("wss://localhost:3000/ws");

// 서버로부터 메시지를 받으면 <div id="messages">에 표시
socket.addEventListener("message", (e) => {
  document
    .getElementById("messages")
    .append(e.data, document.createElement("br"));
});

// 폼 제출 시 input 값을 서버로 전송하고 입력창 비움
const form = document.querySelector("form");
form.addEventListener("submit", () => {
  socket.send(form.elements.namedItem("content").value);
  form.elements.namedItem("content").value = "";
});
