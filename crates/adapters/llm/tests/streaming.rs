use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use margin_ask::{
    ApiFormat, CancellationToken, ChatMessage, Error, LanguageModel, LlmConfig, ToolSpec,
};
use margin_llm::HttpLanguageModel;
use serde_json::{Value, json};

const RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);

struct Stub {
    base_url: String,
    requests: Receiver<String>,
}

fn stub(status: u16, content_type: &str, body: &str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
    let address = listener.local_addr().expect("the stub address");
    let (sender, requests) = channel();
    let response_body = body.to_owned();
    let response_type = content_type.to_owned();
    thread::spawn(move || {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        let request = read_request(&mut socket);
        let _ignored = sender.send(request);
        let head = format!(
            "HTTP/1.1 {status} X\r\ncontent-type: {response_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            response_body.len()
        );
        let _ignored = socket.write_all(head.as_bytes());
        let _ignored = socket.write_all(response_body.as_bytes());
        let _ignored = socket.flush();
    });
    Stub {
        base_url: format!("http://{address}/v1"),
        requests,
    }
}

fn read_request(socket: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        match socket.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
        }
        if complete(&buffer) {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn complete(buffer: &[u8]) -> bool {
    let text = String::from_utf8_lossy(buffer);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let length = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .map(str::to_owned)
        })
        .and_then(|value| value.parse::<usize>().ok());
    length.is_none_or(|length| body.len() >= length)
}

fn config(format: ApiFormat, base_url: &str) -> LlmConfig {
    LlmConfig {
        api_format: format,
        base_url: base_url.to_owned(),
        api_key: "k".to_owned(),
        model: "m".to_owned(),
        effort: None,
    }
}

fn tools() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "search",
        description: "d",
        parameters: json!({"type": "object"}),
    }]
}

fn body_of(request: &str) -> Value {
    let (_, body) = request
        .split_once("\r\n\r\n")
        .expect("the request has a body");
    serde_json::from_str(body).expect("the request body is JSON")
}

#[test]
fn the_client_is_created_outside_a_runtime() {
    let model = HttpLanguageModel::new();

    assert!(format!("{model:?}").starts_with("HttpLanguageModel"));
}

#[tokio::test]
async fn openai_streaming_parses_sse_text_and_split_tool_calls() {
    let stub = stub(
        200,
        "text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Let \"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"me look.\"}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"search\",\"arguments\":\"{\\\"quer\"}}]}}]}\n\n\
         data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"y\\\":\\\"roadmap\\\"}\"}}]}}]}\n\n\
         data: [DONE]\n\n",
    );
    let model = HttpLanguageModel::new();
    let mut streamed = String::new();
    let turn = model
        .stream_chat(
            &config(ApiFormat::Openai, &stub.base_url),
            "sys",
            &[ChatMessage::User("what did I decide?".to_owned())],
            &tools(),
            &CancellationToken::new(),
            &mut |text| streamed.push_str(text),
        )
        .await
        .expect("the recorded stream decodes");

    assert_eq!(turn.text, "Let me look.");
    assert_eq!(streamed, "Let me look.", "every delta reached the caller");
    assert_eq!(turn.calls.len(), 1);
    assert_eq!(turn.calls[0].id, "call_1");
    assert_eq!(turn.calls[0].name, "search");
    assert_eq!(turn.calls[0].args, json!({"query": "roadmap"}));

    let request = stub
        .requests
        .recv_timeout(RECEIVE_TIMEOUT)
        .expect("the stub received a request");
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(request.contains("authorization: Bearer k"), "got {request}");
    let body = body_of(&request);
    assert_eq!(body["model"], "m");
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "sys");
    assert_eq!(body["messages"][1]["content"], "what did I decide?");
    assert_eq!(body["tools"][0]["function"]["name"], "search");
}

#[tokio::test]
async fn a_keyless_endpoint_is_called_without_an_authorization_header() {
    let stub = stub(
        200,
        "text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n",
    );
    let mut config = config(ApiFormat::Openai, &stub.base_url);
    config.api_key = "  ".to_owned();

    HttpLanguageModel::new()
        .stream_chat(
            &config,
            "sys",
            &[ChatMessage::User("hi".to_owned())],
            &tools(),
            &CancellationToken::new(),
            &mut |_text| {},
        )
        .await
        .expect("the recorded stream decodes");

    let request = stub
        .requests
        .recv_timeout(RECEIVE_TIMEOUT)
        .expect("the stub received a request");
    assert!(!request.contains("authorization"), "got {request}");
}

#[tokio::test]
async fn anthropic_streaming_parses_thinking_then_tool_use() {
    let stub = stub(
        200,
        "text/event-stream",
        "event: content_block_start\n\
         data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"Check \"}}\n\n\
         data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"the note.\"}}\n\n\
         data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig\"}}\n\n\
         data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
         data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Looking.\"}}\n\n\
         data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"search\"}}\n\n\
         data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\\\"roadmap\\\"}\"}}\n\n\
         data: {\"type\":\"message_stop\"}\n\n",
    );
    let mut streamed = String::new();
    let turn = HttpLanguageModel::new()
        .stream_chat(
            &config(ApiFormat::Anthropic, &stub.base_url),
            "sys",
            &[ChatMessage::User("what did I decide?".to_owned())],
            &tools(),
            &CancellationToken::new(),
            &mut |text| streamed.push_str(text),
        )
        .await
        .expect("the recorded stream decodes");

    assert_eq!(turn.text, "Looking.");
    assert_eq!(streamed, "Looking.", "thinking is never answer text");
    assert_eq!(turn.reasoning.len(), 1);
    assert_eq!(turn.reasoning[0]["type"], "thinking");
    assert_eq!(turn.reasoning[0]["thinking"], "Check the note.");
    assert_eq!(turn.reasoning[0]["signature"], "sig");
    assert_eq!(turn.calls.len(), 1);
    assert_eq!(turn.calls[0].id, "toolu_1");
    assert_eq!(turn.calls[0].args, json!({"query": "roadmap"}));

    let request = stub
        .requests
        .recv_timeout(RECEIVE_TIMEOUT)
        .expect("the stub received a request");
    assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(
        request.contains("anthropic-version: 2023-06-01"),
        "got {request}"
    );
    assert!(request.contains("x-api-key: k"), "got {request}");
    let body = body_of(&request);
    assert_eq!(body["system"], "sys");
    assert_eq!(body["max_tokens"], 4096);
    assert!(body.get("thinking").is_none(), "no effort, no thinking");
}

#[tokio::test]
async fn a_provider_error_response_becomes_a_typed_http_error() {
    let stub = stub(
        429,
        "application/json",
        "{\"error\":{\"message\":\"slow down\"}}",
    );
    let outcome = HttpLanguageModel::new()
        .stream_chat(
            &config(ApiFormat::Openai, &stub.base_url),
            "sys",
            &[ChatMessage::User("hi".to_owned())],
            &tools(),
            &CancellationToken::new(),
            &mut |_text| {},
        )
        .await;

    assert_eq!(
        outcome.expect_err("a rejected request is an error"),
        Error::Http {
            status: Some(429),
            message: "slow down".to_owned()
        }
    );
}

#[tokio::test]
async fn cancellation_stops_a_stream_that_is_already_running() {
    let stub = stub(
        200,
        "text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"never\"}}]}\n\ndata: [DONE]\n\n",
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let outcome = HttpLanguageModel::new()
        .stream_chat(
            &config(ApiFormat::Openai, &stub.base_url),
            "sys",
            &[ChatMessage::User("hi".to_owned())],
            &tools(),
            &cancellation,
            &mut |_text| {},
        )
        .await;

    assert_eq!(
        outcome.expect_err("a cancelled stream is an error"),
        Error::Cancelled
    );
}

#[tokio::test]
async fn listing_models_sorts_and_deduplicates_the_endpoint_answer() {
    let stub = stub(
        200,
        "application/json",
        "{\"data\":[{\"id\":\"b\"},{\"id\":\"a\"},{\"id\":\"a\"}]}",
    );

    let models = HttpLanguageModel::new()
        .list_models(&config(ApiFormat::Openai, &stub.base_url))
        .await
        .expect("the recorded model list decodes");

    assert_eq!(models, vec!["a".to_owned(), "b".to_owned()]);
    let request = stub
        .requests
        .recv_timeout(RECEIVE_TIMEOUT)
        .expect("the stub received a request");
    assert!(
        request.starts_with("GET /v1/models HTTP/1.1"),
        "got {request}"
    );
}

#[tokio::test]
async fn an_endpoint_without_models_is_an_error() {
    let stub = stub(200, "application/json", "{\"data\":[]}");

    let outcome = HttpLanguageModel::new()
        .list_models(&config(ApiFormat::Openai, &stub.base_url))
        .await;

    assert_eq!(outcome, Err(Error::NoModels));
}
