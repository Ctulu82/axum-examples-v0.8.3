//! Based on tokio-tungstenite example websocket client, but with multiple
//! concurrent websocket clients in one package
//!
//! This will connect to a server specified in the SERVER with N_CLIENTS
//! concurrent connections, and then flood some test messages over websocket.
//! This will also print whatever it gets into stdout.
//!
//! Note that this is not currently optimized for performance, especially around
//! stdout mutex management. Rather it's intended to show an example of working with axum's
//! websocket server and how the client-side and server-side code can be quite similar.
//!

use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use std::ops::ControlFlow;
use std::time::Instant;
use tokio_tungstenite::tungstenite::Utf8Bytes;

// we will use tungstenite for websocket client impl (same library as what axum is using)
// WebSocket 연결용 클라이언트 라이브러리
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message},
};

const N_CLIENTS: usize = 2; //set to desired number
const SERVER: &str = "ws://127.0.0.1:3000/ws";

/// 🧪 main() : N개의 클라이언트 생성

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    //spawn several clients that will concurrently talk to the server
    let mut clients = (0..N_CLIENTS)
        .map(|cli| tokio::spawn(spawn_client(cli)))
        .collect::<FuturesUnordered<_>>();

    //wait for all our clients to exit
    while clients.next().await.is_some() {}

    let end_time = Instant::now();

    //total time should be the same no matter how many clients we spawn
    println!(
        "Total time taken {:#?} with {N_CLIENTS} concurrent clients, should be about 6.45 seconds.",
        end_time - start_time
    );
}

/// 🚀 spawn_client() : 클라이언트가 서버에 연결하고 메시지 송수신

//creates a client. quietly exits on failure.
async fn spawn_client(who: usize) {
    let ws_stream = match connect_async(SERVER).await {
        Ok((stream, response)) => {
            println!("Handshake for client {who} has been completed");
            // This will be the HTTP response, same as with server this is the last moment we
            // can still access HTTP stuff.
            println!("Server response was {response:?}");
            stream
        }
        Err(e) => {
            println!("WebSocket handshake for client {who} failed with {e}!");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    //we can ping the server for start
    // 서버에 Ping 전송
    sender
        .send(Message::Ping(axum::body::Bytes::from_static(
            b"Hello, Server!",
        )))
        .await
        .expect("Can not send!");

    //spawn an async sender to push some more messages into the server
    // 메시지 송신 task
    let mut send_task = tokio::spawn(async move {
        for i in 1..30 {
            // In any websocket error, break loop.
            if sender
                .send(Message::Text(format!("Message number {i}...").into()))
                .await
                .is_err()
            {
                //just as with server, if send fails there is nothing we can do but exit.
                return;
            }

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        // When we are done we may want our client to close connection cleanly.
        println!("Sending close to {who}...");
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Utf8Bytes::from_static("Goodbye"),
            })))
            .await
        {
            println!("Could not send Close due to {e:?}, probably it is ok?");
        };
    });

    //receiver just prints whatever it gets
    // 메시지 수신 task
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // print message and break if instructed to do so
            if process_message(msg, who).is_break() {
                break;
            }
        }
    });

    //wait for either task to finish and kill the other task
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }
}

/// 🧾 메시지 처리 함수

/// Function to handle messages we get (with a slight twist that Frame variant is visible
/// since we are working with the underlying tungstenite library directly without axum here).
fn process_message(msg: Message, who: usize) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} got str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {} got {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} got close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {who} somehow got close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>> {who} got pong with {v:?}");
        }
        // Just as with axum server, the underlying tungstenite websocket library
        // will handle Ping for you automagically by replying with Pong and copying the
        // v according to spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> {who} got ping with {v:?}");
        }

        Message::Frame(_) => {
            unreachable!("This is never supposed to happen")
        }
    }
    ControlFlow::Continue(())
}
