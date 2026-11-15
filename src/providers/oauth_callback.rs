// SPDX-License-Identifier: MPL-2.0

//! Shared pieces of the local OAuth callback listeners (Codex, Gemini,
//! Antigravity, Grok): a bounded request read and an overall deadline, so a
//! silent or stray local connection can never block or end a sign-in.

use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

/// How long a sign-in waits for the browser to come back.
pub const LISTEN_DEADLINE: Duration = Duration::from_secs(300);
/// How long one connection may take to send its request line and headers.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REQUEST_BYTES: usize = 8192;

pub fn deadline() -> Instant {
    Instant::now() + LISTEN_DEADLINE
}

/// Waits for the next connection until `deadline`, then gives up with a
/// "sign-in timed out" error.
pub async fn accept_until(
    listener: &TcpListener,
    deadline: Instant,
    provider: &str,
) -> Result<TcpStream, String> {
    match tokio::time::timeout_at(deadline, listener.accept()).await {
        Ok(Ok((stream, _))) => Ok(stream),
        Ok(Err(error)) => Err(format!(
            "failed to receive {provider} OAuth callback: {error}"
        )),
        Err(_) => Err(format!(
            "{provider} sign-in timed out after {} minutes",
            LISTEN_DEADLINE.as_secs() / 60
        )),
    }
}

/// Reads one HTTP request up to the end of its headers, at most 8 KiB, within
/// `READ_TIMEOUT`. A connection that is silent, slow, closed or broken yields
/// an empty request, which the caller answers as a bad request and skips, so
/// the listener keeps waiting for the real callback.
pub async fn read_http_request(stream: &mut TcpStream) -> String {
    read_http_request_within(stream, READ_TIMEOUT).await
}

async fn read_http_request_within(stream: &mut TcpStream, limit: Duration) -> String {
    let mut buffer = Vec::with_capacity(1024);
    let read = tokio::time::timeout(limit, async {
        let mut chunk = [0u8; 1024];
        loop {
            let bytes = stream.read(&mut chunk).await?;
            if bytes == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..bytes]);
            if buffer.len() >= MAX_REQUEST_BYTES || buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        Ok::<(), std::io::Error>(())
    })
    .await;
    match read {
        Ok(Ok(())) => String::from_utf8_lossy(&buffer).into_owned(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    async fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (client, server)
    }

    #[tokio::test]
    async fn reads_a_request_split_across_segments() {
        let (mut client, mut server) = pair().await;
        client.write_all(b"GET /callback?code=1").await.unwrap();
        client.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        client
            .write_all(b"&state=s HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let request = read_http_request(&mut server).await;
        assert!(request.starts_with("GET /callback?code=1&state=s HTTP/1.1"));
    }

    #[tokio::test]
    async fn a_silent_connection_times_out_instead_of_blocking() {
        let (_client, mut server) = pair().await;
        let request = read_http_request_within(&mut server, Duration::from_millis(50)).await;
        assert!(request.is_empty());
    }

    #[tokio::test]
    async fn waiting_for_the_browser_has_an_overall_deadline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let soon = Instant::now() + Duration::from_millis(50);
        let error = accept_until(&listener, soon, "Test").await.unwrap_err();
        assert!(error.contains("timed out"));
    }
}
