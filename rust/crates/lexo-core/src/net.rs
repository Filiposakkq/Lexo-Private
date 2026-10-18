//! The one place that talks to the network: a POST request for the machine translation (port of `_post` of `core/lexo/mt.py`).
//!
//! `mt` builds the request and reads the answer; this is the function it is given (`mt::Http`). A status of 400 or more is an answer, not a failure (the caller turns it into a message);
//! only a request that did not get an answer at all (no connection, a timeout, a bad certificate) is an `MtError`.
use crate::messages::msg;
use crate::mt::MtError;
use serde_json::json;
use std::io::Read;
use std::time::Duration;

/// Seconds to wait for a service (the same as `TIMEOUT` of the Python code).
const TIMEOUT: u64 = 30;
/// The longest answer that is read; a translation is a few kilobytes.
const MAX_BODY: u64 = 16 * 1024 * 1024;

fn failed(reason: String) -> MtError {
    MtError(msg("The connection to the service failed: {reason}", &[("reason", json!(reason))]))
}

/// Sends `data` with POST to `url` and returns the status and the text of the answer.
pub fn post(url: &str, headers: &[(String, String)], data: &[u8]) -> Result<(u16, String), MtError> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(TIMEOUT)).redirects(0).build();
    let mut req = agent.post(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.send_bytes(data) {
        Ok(resp) => read(resp),
        Err(ureq::Error::Status(_, resp)) => read(resp),
        Err(ureq::Error::Transport(t)) => Err(failed(t.to_string())),
    }
}

fn read(resp: ureq::Response) -> Result<(u16, String), MtError> {
    let status = resp.status();
    let mut bytes = Vec::new();
    resp.into_reader().take(MAX_BODY).read_to_end(&mut bytes).map_err(|e| failed(e.to_string()))?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// A server that answers once with `answer` and hands back the request it got (head and body).
    fn serve_once(answer: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/translate", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let (mut head, mut length) = (String::new(), 0usize);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                if line.trim().is_empty() {
                    break;
                }
                head.push_str(&line);
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).unwrap();
            stream.write_all(answer.as_bytes()).unwrap();
            head + &String::from_utf8(body).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn it_sends_the_headers_and_the_body_and_returns_the_answer() {
        let (url, server) = serve_once("HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"text\":\"ahoj\"}");
        let (status, body) =
            post(&url, &[("Authorization".into(), "Bearer k".into()), ("Content-Type".into(), "application/json".into())], b"{\"q\":1}").unwrap();
        assert_eq!((status, body.as_str()), (200, "{\"text\":\"ahoj\"}"));
        let got = server.join().unwrap();
        assert!(got.starts_with("POST /translate"), "{got}");
        assert!(got.to_ascii_lowercase().contains("authorization: bearer k"), "{got}");
        assert!(got.ends_with("{\"q\":1}"), "{got}");
    }

    #[test]
    fn an_error_status_is_an_answer() {
        let (url, server) = serve_once("HTTP/1.1 403 Forbidden\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope");
        assert_eq!(post(&url, &[], b"x").unwrap(), (403, "nope".to_string()));
        server.join().unwrap();
    }

    /// Needs the internet: `cargo test -p lexo-core -- --ignored https`. Any answer, even an error status, proves that the certificate was checked and the TLS worked.
    #[test]
    #[ignore]
    fn https_works_against_a_real_server() {
        let (status, body) = post("https://example.com/", &[], b"x").unwrap();
        assert!(status > 0 && !body.is_empty(), "{status}");
    }

    #[test]
    fn no_connection_is_a_message_for_the_user() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port(); // free again after the drop
        let e = post(&format!("http://127.0.0.1:{port}/"), &[], b"x").unwrap_err();
        assert!(e.0.text().contains("The connection to the service failed"), "{}", e.0.text());
    }
}
