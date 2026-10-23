//! The HTTP seam: the real client, and a scripted one for tests that keeps every request it was
//! sent — encoded, so a request the API would reject fails the test that built it.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};

/// A response body, chunk by chunk.
pub type Body = Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>;

/// A request on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Where it goes.
    pub url: String,
    /// Its headers, secrets included — never logged.
    pub headers: Vec<(String, String)>,
    /// Its JSON body.
    pub body: Vec<u8>,
}

/// A response's head and its streaming body.
pub struct HttpResponse {
    /// The status code.
    pub status: u16,
    /// Headers, names lower-cased.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Body,
}

impl HttpResponse {
    /// A header's value, by lower-case name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Something that sends a request and returns its response head once it arrives.
pub trait Transport: Send + Sync {
    /// Sends `request`.
    fn send(
        &self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<HttpResponse>> + Send + '_>>;
}

/// The real transport: reqwest over rustls, trusting the platform's certificate store (private
/// CAs included).
pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    /// A transport whose connections time out after `connect`.
    ///
    /// # Errors
    ///
    /// The TLS stack cannot be initialised.
    pub fn new(connect: Duration) -> io::Result<HttpTransport> {
        let client = reqwest::Client::builder()
            .connect_timeout(connect)
            .build()
            .map_err(io::Error::other)?;
        Ok(HttpTransport { client })
    }
}

impl Transport for HttpTransport {
    fn send(
        &self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<HttpResponse>> + Send + '_>> {
        Box::pin(async move {
            let mut builder = self.client.post(&request.url).body(request.body);
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            let response = builder.send().await.map_err(io::Error::other)?;
            let status = response.status().as_u16();
            let headers = response
                .headers()
                .iter()
                .map(|(n, v)| {
                    (
                        n.as_str().to_ascii_lowercase(),
                        v.to_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect();
            let body = response
                .bytes_stream()
                .map(|chunk| chunk.map_err(io::Error::other));
            Ok(HttpResponse {
                status,
                headers,
                body: Box::pin(body),
            })
        })
    }
}

/// One scripted reply.
#[derive(Debug, Clone)]
pub enum Reply {
    /// The connection fails before any response.
    ConnectError,
    /// A response: its status, headers, and body chunks, each sent after its delay.
    Respond {
        /// The status code.
        status: u16,
        /// Headers, names lower-cased.
        headers: Vec<(String, String)>,
        /// Body chunks and the wait before each.
        chunks: Vec<(Duration, Vec<u8>)>,
    },
}

impl Reply {
    /// A 200 whose body is `text`, sent at once in one chunk.
    #[must_use]
    pub fn ok(text: &str) -> Reply {
        Reply::Respond {
            status: 200,
            headers: Vec::new(),
            chunks: vec![(Duration::ZERO, text.as_bytes().to_vec())],
        }
    }

    /// An error status with a JSON body and optional headers.
    #[must_use]
    pub fn status(status: u16, body: &str, headers: &[(&str, &str)]) -> Reply {
        Reply::Respond {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            chunks: vec![(Duration::ZERO, body.as_bytes().to_vec())],
        }
    }
}

/// A transport that answers from a script and keeps every request it was sent.
#[derive(Default)]
pub struct Scripted {
    replies: Mutex<VecDeque<Reply>>,
    sent: Mutex<Vec<HttpRequest>>,
}

impl Scripted {
    /// A transport that will answer with `replies`, in order.
    #[must_use]
    pub fn new(replies: impl IntoIterator<Item = Reply>) -> Scripted {
        Scripted {
            replies: Mutex::new(replies.into_iter().collect()),
            sent: Mutex::new(Vec::new()),
        }
    }

    /// The requests sent so far.
    ///
    /// # Panics
    ///
    /// The lock was poisoned by a panicking test.
    #[must_use]
    pub fn sent(&self) -> Vec<HttpRequest> {
        self.sent.lock().expect("lock").clone()
    }
}

impl Transport for Scripted {
    fn send(
        &self,
        request: HttpRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<HttpResponse>> + Send + '_>> {
        self.sent.lock().expect("lock").push(request);
        let reply = self.replies.lock().expect("lock").pop_front();
        Box::pin(async move {
            match reply {
                None => Err(io::Error::other("the script has no more replies")),
                Some(Reply::ConnectError) => Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "scripted connect error",
                )),
                Some(Reply::Respond {
                    status,
                    headers,
                    chunks,
                }) => {
                    let body =
                        futures_util::stream::iter(chunks).then(|(wait, bytes)| async move {
                            tokio::time::sleep(wait).await;
                            Ok(Bytes::from(bytes))
                        });
                    Ok(HttpResponse {
                        status,
                        headers,
                        body: Box::pin(body),
                    })
                }
            }
        })
    }
}
