use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::error::{is_timeout, Error};

const MAX_HEAD: usize = 16 * 1024;
const STATUS_BODY_CAP: usize = 4 * 1024;

pub(crate) struct RawRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub host_header: &'a str,
    pub authorization: Option<&'a str>,
    pub body: Option<&'a [u8]>,
}

pub(crate) struct RawResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub(crate) fn exchange(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
    max_body: usize,
    request: &RawRequest<'_>,
) -> Result<RawResponse, Error> {
    let addr = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(Error::Connect)?
        .next()
        .ok_or_else(|| Error::Config(format!("no address for {host}:{port}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, connect_timeout).map_err(Error::Connect)?;
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(Error::Io)?;
    stream
        .set_write_timeout(Some(read_timeout))
        .map_err(Error::Io)?;

    let bytes = encode(request);
    stream.write_all(&bytes).map_err(map_io)?;

    let (status, rest) = read_head(&mut stream, max_body)?;
    if (300..400).contains(&status) {
        return Err(Error::Redirect { status });
    }
    let body = read_body(&mut stream, &rest.headers, rest.already, max_body)?;
    Ok(RawResponse { status, body })
}

struct HeadRest {
    headers: Vec<(String, String)>,
    already: Vec<u8>,
}

fn encode(request: &RawRequest<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(256 + request.body.map(|b| b.len()).unwrap_or(0));
    push(
        &mut out,
        format!("{} {} HTTP/1.1\r\n", request.method, request.path),
    );
    push(&mut out, format!("Host: {}\r\n", request.host_header));
    push(&mut out, "Connection: close\r\n");
    push(&mut out, "Accept: application/json\r\n");
    push(
        &mut out,
        format!("User-Agent: roncho/{}\r\n", env!("CARGO_PKG_VERSION")),
    );
    if let Some(auth) = request.authorization {
        push(&mut out, format!("Authorization: Bearer {auth}\r\n"));
    }
    if let Some(body) = request.body {
        push(&mut out, "Content-Type: application/json\r\n");
        push(&mut out, format!("Content-Length: {}\r\n", body.len()));
    }
    push(&mut out, "\r\n");
    if let Some(body) = request.body {
        out.extend_from_slice(body);
    }
    out
}

fn push(out: &mut Vec<u8>, text: impl AsRef<[u8]>) {
    out.extend_from_slice(text.as_ref());
}

fn read_head(stream: &mut TcpStream, max_body: usize) -> Result<(u16, HeadRest), Error> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            let head = parse_head(&buf[..pos])?;
            let already = buf[pos + 4..].to_vec();
            if already.len() > max_body {
                return Err(Error::TooLarge);
            }
            return Ok((
                head.0,
                HeadRest {
                    headers: head.1,
                    already,
                },
            ));
        }
        if buf.len() > MAX_HEAD {
            return Err(Error::TooLarge);
        }
        match stream.read(&mut tmp) {
            Ok(0) => return Err(Error::Closed),
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(err) if is_timeout(&err) => return Err(Error::Timeout),
            Err(err) => return Err(Error::Io(err)),
        }
    }
}

fn parse_head(bytes: &[u8]) -> Result<(u16, Vec<(String, String)>), Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Decode("response head".into()))?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let mut parts = status_line.split_whitespace();
    let version = parts.next();
    let code = parts.next();
    if version != Some("HTTP/1.1") && version != Some("HTTP/1.0") {
        return Err(Error::Decode("status line".into()));
    }
    let status: u16 = code
        .and_then(|c| c.parse().ok())
        .filter(|c| (100..=599).contains(c))
        .ok_or_else(|| Error::Decode("status line".into()))?;
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::Decode("header".into()))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    Ok((status, headers))
}

fn read_body(
    stream: &mut TcpStream,
    headers: &[(String, String)],
    already: Vec<u8>,
    max_body: usize,
) -> Result<Vec<u8>, Error> {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        return read_chunked(stream, already, max_body);
    }
    if let Some(len) = header("content-length") {
        let len: usize = len
            .parse()
            .map_err(|_| Error::Decode("content-length".into()))?;
        if len > max_body {
            return Err(Error::TooLarge);
        }
        let mut body = already;
        if body.len() > len {
            body.truncate(len);
            return Ok(body);
        }
        body.reserve(len - body.len());
        while body.len() < len {
            let mut tmp = [0u8; 8192];
            match stream.read(&mut tmp) {
                Ok(0) => return Err(Error::Closed),
                Ok(n) => {
                    let room = len - body.len();
                    let take = n.min(room);
                    body.extend_from_slice(&tmp[..take]);
                }
                Err(err) if is_timeout(&err) => return Err(Error::Timeout),
                Err(err) => return Err(Error::Io(err)),
            }
        }
        return Ok(body);
    }
    let mut body = already;
    let mut tmp = [0u8; 8192];
    loop {
        if body.len() > max_body {
            return Err(Error::TooLarge);
        }
        match stream.read(&mut tmp) {
            Ok(0) => return Ok(body),
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(err) if is_timeout(&err) => return Err(Error::Timeout),
            Err(err) => return Err(Error::Io(err)),
        }
    }
}

fn read_chunked(
    stream: &mut TcpStream,
    mut buf: Vec<u8>,
    max_body: usize,
) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        let Some(pos) = find(&buf, b"\r\n") else {
            if buf.len() > 64 {
                return Err(Error::Decode("chunk size".into()));
            }
            match stream.read(&mut tmp) {
                Ok(0) => return Err(Error::Closed),
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(err) if is_timeout(&err) => return Err(Error::Timeout),
                Err(err) => return Err(Error::Io(err)),
            }
            continue;
        };
        let size_bytes = &buf[..pos];
        let size = parse_chunk_size(size_bytes)?;
        buf.drain(..pos + 2);
        if size == 0 {
            return Ok(out);
        }
        if out.len().saturating_add(size) > max_body {
            return Err(Error::TooLarge);
        }
        while buf.len() < size + 2 {
            match stream.read(&mut tmp) {
                Ok(0) => return Err(Error::Closed),
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(err) if is_timeout(&err) => return Err(Error::Timeout),
                Err(err) => return Err(Error::Io(err)),
            }
        }
        out.extend_from_slice(&buf[..size]);
        if &buf[size..size + 2] != b"\r\n" {
            return Err(Error::Decode("chunk".into()));
        }
        buf.drain(..size + 2);
    }
}

fn parse_chunk_size(token: &[u8]) -> Result<usize, Error> {
    let semi = token.iter().position(|&b| b == b';').unwrap_or(token.len());
    let hex = &token[..semi];
    if hex.is_empty() || hex.iter().any(|b| !b.is_ascii_hexdigit()) {
        return Err(Error::Decode("chunk size".into()));
    }
    let text = std::str::from_utf8(hex).map_err(|_| Error::Decode("chunk size".into()))?;
    usize::from_str_radix(text, 16).map_err(|_| Error::Decode("chunk size".into()))
}

pub(crate) fn body_prefix(body: &[u8]) -> String {
    let end = body.len().min(STATUS_BODY_CAP);
    String::from_utf8_lossy(&body[..end]).into_owned()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn map_io(err: std::io::Error) -> Error {
    if is_timeout(&err) {
        Error::Timeout
    } else {
        Error::Io(err)
    }
}
