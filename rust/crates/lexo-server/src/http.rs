//! The little of HTTP/1.1 the local server needs: read one request, write one response, close. Nothing here knows about Lexo.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;

/// A request: method, the raw target (`/api/entries?limit=5`) and the headers (names in lower case).
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    stream: TcpStream,
    /// Bytes that came with the head, after the blank line.
    pending: Vec<u8>,
}

const MAX_HEAD: usize = 64 * 1024;
/// Large single writes stalled intermittently on Windows loopback, so bodies are written in small chunks.
const CHUNK: usize = 16 * 1024;

impl Request {
    /// Read the request line and the headers. `None` when the connection gave nothing usable.
    pub fn read(mut stream: TcpStream) -> Option<Request> {
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(60)));
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        let end = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            if buf.len() > MAX_HEAD {
                return None;
            }
            let n = stream.read(&mut chunk).ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..end]).into_owned();
        let pending = buf[end + 4..].to_vec();
        let mut lines = head.split("\r\n");
        let mut first = lines.next()?.split(' ');
        let (method, target) = (first.next()?.to_string(), first.next()?.to_string());
        let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())).collect();
        Some(Request { method, target, headers, stream, pending })
    }

    pub fn header(&self, name: &str) -> &str {
        self.headers.get(name).map_or("", String::as_str)
    }

    pub fn content_length(&self) -> Option<usize> {
        match self.headers.get("content-length") {
            None => Some(0),
            Some(v) => v.trim().parse().ok(),
        }
    }

    /// The body (`n` bytes).
    pub fn body(&mut self, n: usize) -> Option<Vec<u8>> {
        let mut body = std::mem::take(&mut self.pending);
        body.truncate(n);
        let mut chunk = [0u8; 8192];
        while body.len() < n {
            let want = (n - body.len()).min(chunk.len());
            let got = self.stream.read(&mut chunk[..want]).ok()?;
            if got == 0 {
                return None;
            }
            body.extend_from_slice(&chunk[..got]);
        }
        Some(body)
    }

    /// Read and discard the body before an early answer: closing a socket that still has unread data makes Windows reset the connection, so the client sometimes never sees
    /// the 403 / 413.
    pub fn drain(&mut self, limit: usize) {
        if let Some(n) = self.content_length().filter(|n| *n <= limit) {
            let _ = self.body(n);
        }
    }

    pub fn respond(&mut self, code: u16, ctype: &str, body: &[u8], cache: bool) {
        let reason = match code {
            200 => "OK",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            409 => "Conflict",
            413 => "Payload Too Large",
            _ => "Internal Server Error",
        };
        let head = format!(
            "HTTP/1.1 {code} {reason}\r\nServer: Lexo\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: {}\r\nX-Content-Type-Options: nosniff\r\n\
             Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:\r\nConnection: close\r\n\r\n",
            body.len(),
            if cache { "private, max-age=600" } else { "no-store" }
        );
        let _ = self.stream.write_all(head.as_bytes());
        for part in body.chunks(CHUNK) {
            if self.stream.write_all(part).is_err() {
                return;
            }
        }
        let _ = self.stream.flush();
    }
}

/// `%XX` and (in a query) `+` decoded; bytes that are not UTF-8 become the replacement character.
pub fn percent_decode(s: &str, plus_is_space: bool) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'%'));
                i += 3;
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `parse_qs`: the first value of every name, names with an empty value left out.
pub fn parse_query(query: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let (k, v) = (percent_decode(k, true), percent_decode(v, true));
        if !v.is_empty() && !out.iter().any(|(name, _)| *name == k) {
            out.push((k, v));
        }
    }
    out
}

/// The type of a file of the UI, by its name.
pub fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_decoded_like_parse_qs() {
        assert_eq!(parse_query("a=1&b=&c=x%20y+z&a=2"), vec![("a".to_string(), "1".to_string()), ("c".to_string(), "x y z".to_string())]);
        assert_eq!(percent_decode("%C5%99%zz%4", false), "ř%zz%4");
    }
}
