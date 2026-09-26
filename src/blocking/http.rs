use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::error::{is_timeout, Error};

#[cfg(feature = "tls")]
use super::tls;

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

pub(crate) struct Endpoint<'a> {
    pub host: &'a str,
    pub port: u16,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub max_body: usize,
    #[cfg(feature = "tls")]
    pub tls: Option<&'a std::sync::Arc<rustls::ClientConfig>>,
}

pub(crate) fn exchange(
    endpoint: &Endpoint<'_>,
    request: &RawRequest<'_>,
) -> Result<RawResponse, Error> {
    let mut stream = dial(endpoint)?;
    stream.write_all(&encode(request)).map_err(map_io)?;
    let (status, rest) = read_head(&mut stream, endpoint.max_body)?;
    if (300..400).contains(&status) {
        return Err(Error::Redirect { status });
    }
    let body = read_body(&mut stream, &rest.headers, rest.already, endpoint.max_body)?;
    Ok(RawResponse { status, body })
}

/// Send one request and return the body as a pull source. A non-2xx status is
/// still an error, after the body is read up to the cap.
pub(crate) fn open(endpoint: &Endpoint<'_>, request: &RawRequest<'_>) -> Result<LiveBody, Error> {
    let mut stream = dial(endpoint)?;
    stream.write_all(&encode(request)).map_err(map_io)?;
    let (status, rest) = read_head(&mut stream, endpoint.max_body)?;
    if (300..400).contains(&status) {
        return Err(Error::Redirect { status });
    }
    if !(200..300).contains(&status) {
        let body = read_body(&mut stream, &rest.headers, rest.already, endpoint.max_body)?;
        return Err(match status {
            401 | 403 => Error::Unauthorized { status },
            _ => Error::Status {
                status,
                body: body_prefix(&body),
            },
        });
    }
    LiveBody::new(stream, rest, endpoint.max_body)
}

enum Conn {
    Plain(TcpStream),
    #[cfg(feature = "tls")]
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(stream) => stream.read(buf),
            #[cfg(feature = "tls")]
            Conn::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(stream) => stream.write(buf),
            #[cfg(feature = "tls")]
            Conn::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(stream) => stream.flush(),
            #[cfg(feature = "tls")]
            Conn::Tls(stream) => stream.flush(),
        }
    }
}

fn dial(endpoint: &Endpoint<'_>) -> Result<Conn, Error> {
    // Name resolution is not bounded by the connect timeout. Each resolved
    // address gets its own connect attempt: a tailnet name can be IPv6 and IPv4.
    let addrs: Vec<_> = format!("{}:{}", endpoint.host, endpoint.port)
        .to_socket_addrs()
        .map_err(Error::Connect)?
        .collect();
    if addrs.is_empty() {
        return Err(Error::Config(format!(
            "no address for {}:{}",
            endpoint.host, endpoint.port
        )));
    }
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, endpoint.connect_timeout) {
            Ok(stream) => return finish_conn(endpoint, stream),
            Err(err) => last = Some(err),
        }
    }
    Err(Error::Connect(last.expect("at least one address")))
}

fn finish_conn(endpoint: &Endpoint<'_>, stream: TcpStream) -> Result<Conn, Error> {
    stream
        .set_read_timeout(Some(endpoint.read_timeout))
        .map_err(Error::Io)?;
    stream
        .set_write_timeout(Some(endpoint.read_timeout))
        .map_err(Error::Io)?;
    #[cfg(feature = "tls")]
    if let Some(config) = endpoint.tls {
        return Ok(Conn::Tls(Box::new(tls::wrap(
            stream,
            endpoint.host,
            config,
        )?)));
    }
    Ok(Conn::Plain(stream))
}

enum Kind {
    Length { left: usize },
    Chunked,
    Close,
}

enum ChunkPhase {
    Size,
    Data,
    Crlf,
}

pub(crate) struct LiveBody {
    stream: Conn,
    raw: Vec<u8>,
    kind: Kind,
    phase: ChunkPhase,
    chunk_left: usize,
    produced: usize,
    max_body: usize,
    finished: bool,
}

impl LiveBody {
    fn new(stream: Conn, rest: HeadRest, max_body: usize) -> Result<Self, Error> {
        let kind = if rest
            .headers
            .iter()
            .any(|(k, v)| k == "transfer-encoding" && v.eq_ignore_ascii_case("chunked"))
        {
            Kind::Chunked
        } else if let Some((_, len)) = rest.headers.iter().find(|(k, _)| k == "content-length") {
            let left: usize = len
                .parse()
                .map_err(|_| Error::Decode("content-length".into()))?;
            if left > max_body {
                return Err(Error::TooLarge);
            }
            Kind::Length { left }
        } else {
            Kind::Close
        };
        Ok(Self {
            stream,
            raw: rest.already,
            kind,
            phase: ChunkPhase::Size,
            chunk_left: 0,
            produced: 0,
            max_body,
            finished: false,
        })
    }

    /// Next decoded body bytes. `Ok(None)` is a clean end.
    pub(crate) fn read_some(&mut self) -> Result<Option<Vec<u8>>, Error> {
        if self.finished {
            return Ok(None);
        }
        let mut out = Vec::new();
        loop {
            if !out.is_empty() {
                return Ok(Some(out));
            }
            if self.finished {
                return Ok(None);
            }
            self.pump(&mut out)?;
        }
    }

    fn pump(&mut self, out: &mut Vec<u8>) -> Result<(), Error> {
        match self.kind {
            Kind::Length { left } => {
                if left == 0 {
                    self.finished = true;
                    return Ok(());
                }
                if self.raw.is_empty() {
                    self.fill_raw()?;
                }
                let take = self.raw.len().min(left);
                self.take(out, take)?;
                if let Kind::Length { left } = &mut self.kind {
                    *left -= take;
                    if *left == 0 {
                        self.finished = true;
                    }
                }
                Ok(())
            }
            Kind::Close => {
                if self.raw.is_empty() {
                    match self.fill_raw() {
                        Ok(()) => {}
                        Err(Error::Closed) => {
                            self.finished = true;
                            return Ok(());
                        }
                        Err(err) => return Err(err),
                    }
                }
                let take = self.raw.len();
                self.take(out, take)
            }
            Kind::Chunked => self.pump_chunk(out),
        }
    }

    fn pump_chunk(&mut self, out: &mut Vec<u8>) -> Result<(), Error> {
        let mut tmp = [0u8; 8192];
        match self.phase {
            ChunkPhase::Size => {
                let Some(pos) = find(&self.raw, b"\r\n") else {
                    if self.raw.len() > 64 {
                        return Err(Error::Decode("chunk size".into()));
                    }
                    return self.read_socket(&mut tmp);
                };
                let size = parse_chunk_size(&self.raw[..pos])?;
                self.raw.drain(..pos + 2);
                if size == 0 {
                    self.finished = true;
                } else {
                    self.chunk_left = size;
                    self.phase = ChunkPhase::Data;
                }
                Ok(())
            }
            ChunkPhase::Data => {
                if self.raw.is_empty() {
                    return self.read_socket(&mut tmp);
                }
                let take = self.raw.len().min(self.chunk_left);
                self.take(out, take)?;
                self.chunk_left -= take;
                if self.chunk_left == 0 {
                    self.phase = ChunkPhase::Crlf;
                }
                Ok(())
            }
            ChunkPhase::Crlf => {
                if self.raw.len() < 2 {
                    return self.read_socket(&mut tmp);
                }
                if &self.raw[..2] != b"\r\n" {
                    return Err(Error::Decode("chunk".into()));
                }
                self.raw.drain(..2);
                self.phase = ChunkPhase::Size;
                Ok(())
            }
        }
    }

    fn take(&mut self, out: &mut Vec<u8>, n: usize) -> Result<(), Error> {
        if self.produced.saturating_add(n) > self.max_body {
            return Err(Error::TooLarge);
        }
        out.extend_from_slice(&self.raw[..n]);
        self.raw.drain(..n);
        self.produced += n;
        Ok(())
    }

    fn fill_raw(&mut self) -> Result<(), Error> {
        let mut tmp = [0u8; 8192];
        self.read_socket(&mut tmp)
    }

    fn read_socket(&mut self, tmp: &mut [u8]) -> Result<(), Error> {
        match self.stream.read(tmp) {
            Ok(0) => Err(Error::Closed),
            Ok(n) => {
                self.raw.extend_from_slice(&tmp[..n]);
                Ok(())
            }
            Err(err) if is_timeout(&err) => Err(Error::Timeout),
            Err(err) => Err(Error::Io(err)),
        }
    }
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

fn read_head(stream: &mut Conn, max_body: usize) -> Result<(u16, HeadRest), Error> {
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
    stream: &mut Conn,
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
    if body.len() > max_body {
        return Err(Error::TooLarge);
    }
    let mut tmp = [0u8; 8192];
    loop {
        let room = max_body - body.len();
        let want = room.saturating_add(1).min(tmp.len());
        match stream.read(&mut tmp[..want]) {
            Ok(0) => return Ok(body),
            Ok(n) => {
                body.extend_from_slice(&tmp[..n]);
                if body.len() > max_body {
                    return Err(Error::TooLarge);
                }
            }
            Err(err) if is_timeout(&err) => return Err(Error::Timeout),
            Err(err) => return Err(Error::Io(err)),
        }
    }
}

fn read_chunked(stream: &mut Conn, mut buf: Vec<u8>, max_body: usize) -> Result<Vec<u8>, Error> {
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
