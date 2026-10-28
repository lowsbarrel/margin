use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::Mutex;

const IDLE_POLL: Duration = Duration::from_millis(5);
const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub struct S3Stub {
    port: u16,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
    objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl fmt::Debug for S3Stub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Stub")
            .field("port", &self.port)
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl S3Stub {
    #[must_use]
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port binds");
        let port = listener
            .local_addr()
            .expect("the listener reports its address")
            .port();
        let objects = Arc::new(Mutex::new(BTreeMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let bucket = String::from("margin-stub");
        let server = Server {
            bucket: bucket.clone(),
            objects: Arc::clone(&objects),
            stop: Arc::clone(&stop),
        };
        let handle = thread::spawn(move || server.run(&listener));
        Self {
            port,
            bucket,
            access_key: String::from("AKIDEXAMPLE"),
            secret_key: String::from("stub-secret-key"),
            region: String::from("us-east-1"),
            objects,
            stop,
            handle: Some(handle),
        }
    }

    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    #[must_use]
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    #[must_use]
    pub fn access_key(&self) -> &str {
        &self.access_key
    }

    #[must_use]
    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }

    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    #[must_use]
    pub fn object_count(&self) -> usize {
        self.objects.lock().len()
    }

    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.objects.lock().keys().cloned().collect()
    }
}

impl Drop for S3Stub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap_or(());
        }
    }
}

struct Server {
    bucket: String,
    objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    stop: Arc<AtomicBool>,
}

impl Server {
    fn run(&self, listener: &TcpListener) {
        listener
            .set_nonblocking(true)
            .expect("a loopback listener is non-blocking");
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return;
            }
            match listener.accept() {
                Ok((stream, _)) => self.handle(stream),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(IDLE_POLL);
                }
                Err(_) => return,
            }
        }
    }

    fn handle(&self, mut stream: TcpStream) {
        if stream.set_nonblocking(false).is_err() {
            return;
        }
        drop(stream.set_read_timeout(Some(READ_TIMEOUT)));
        let Ok(clone) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(clone);
        let Some(request) = read_request(&mut reader) else {
            return;
        };
        let response = self.route(&request);
        drop(stream.write_all(&response));
        drop(stream.flush());
    }

    fn route(&self, request: &Request) -> Vec<u8> {
        if !request.authorized {
            return respond(403, "Forbidden", 0, &[]);
        }
        let (path, query) = split_target(&request.target);
        let root = format!("/{}", self.bucket);
        if path == root {
            if request.method == "GET" && query.split('&').any(|pair| pair == "list-type=2") {
                return self.list(query);
            }
            return respond(400, "Bad Request", 0, &[]);
        }
        let Some(key) = path.strip_prefix(&format!("{root}/")) else {
            return respond(400, "Bad Request", 0, &[]);
        };
        match request.method.as_str() {
            "PUT" => {
                self.objects
                    .lock()
                    .insert(key.to_owned(), request.body.clone());
                respond(200, "OK", 0, &[])
            }
            "GET" => match self.objects.lock().get(key) {
                Some(body) => respond(200, "OK", body.len(), body),
                None => respond(404, "Not Found", 0, &[]),
            },
            "HEAD" => match self.objects.lock().get(key) {
                Some(body) => respond(200, "OK", body.len(), &[]),
                None => respond(404, "Not Found", 0, &[]),
            },
            "DELETE" => {
                self.objects.lock().remove(key);
                respond(204, "No Content", 0, &[])
            }
            _ => respond(400, "Bad Request", 0, &[]),
        }
    }

    fn list(&self, query: &str) -> Vec<u8> {
        let prefix = query_param(query, "prefix").unwrap_or_default();
        let entries: Vec<String> = self
            .objects
            .lock()
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, body)| {
                format!(
                    "<Contents><Key>{key}</Key><Size>{}</Size></Contents>",
                    body.len()
                )
            })
            .collect();
        let count = entries.len();
        let contents = entries.join("");
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><Name>{}</Name><Prefix>{prefix}</Prefix><KeyCount>{count}</KeyCount><MaxKeys>1000</MaxKeys><IsTruncated>false</IsTruncated>{contents}</ListBucketResult>",
            self.bucket
        );
        respond(200, "OK", body.len(), body.as_bytes())
    }
}

struct Request {
    method: String,
    target: String,
    authorized: bool,
    body: Vec<u8>,
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let mut content_length = 0_usize;
    let mut authorized = false;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            break;
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            match name.to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.trim().parse().unwrap_or(0),
                "authorization" => {
                    authorized = value.trim_start().starts_with("AWS4-HMAC-SHA256");
                }
                _ => {}
            }
        }
    }
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).ok()?;
    }
    Some(Request {
        method,
        target,
        authorized,
        body,
    })
}

fn split_target(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| percent_decode(value))
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
        {
            out.push(high * 16 + low);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn respond(status: u16, reason: &str, content_length: usize, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}
