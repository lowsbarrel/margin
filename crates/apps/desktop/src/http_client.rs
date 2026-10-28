use gpui_kit::http_client::http::HeaderValue;
use gpui_kit::http_client::{AsyncBody, HttpClient, Inner, Request, Response, Result, Url, anyhow};
use margin_kernel::BoxFuture;
use reqwest::{Client, RequestBuilder, header};
use tokio::runtime::Handle;

pub(crate) struct ReqwestHttpClient {
    client: Client,
    runtime: Handle,
    user_agent: HeaderValue,
}

impl ReqwestHttpClient {
    pub(crate) fn new(runtime: Handle) -> Self {
        Self {
            client: Client::new(),
            runtime,
            user_agent: HeaderValue::from_static(concat!("Margin/", env!("CARGO_PKG_VERSION"))),
        }
    }
}

impl HttpClient for ReqwestHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<'static, Result<Response<AsyncBody>>> {
        let (parts, body) = req.into_parts();
        let body = match body.0 {
            Inner::Empty => Vec::new(),
            Inner::Bytes(cursor) => cursor.into_inner().to_vec(),
            Inner::AsyncReader(_) => {
                return Box::pin(async {
                    Err(anyhow!("streaming request bodies are not supported"))
                });
            }
        };
        let request = self
            .client
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .header(header::USER_AGENT, self.user_agent.clone())
            .body(body);
        let response = self.runtime.spawn(fetch(request));
        Box::pin(async move { response.await? })
    }
}

async fn fetch(request: RequestBuilder) -> Result<Response<AsyncBody>> {
    let response = request.send().await?;
    let mut builder = Response::builder().status(response.status());
    if let Some(headers) = builder.headers_mut() {
        headers.extend(response.headers().clone());
    }
    let bytes = response.bytes().await?;
    Ok(builder.body(AsyncBody::from(bytes))?)
}
