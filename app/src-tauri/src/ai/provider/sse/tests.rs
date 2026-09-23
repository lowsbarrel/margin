use super::*;

#[test]
fn decoder_survives_payloads_split_across_chunks() {
    let mut decoder = SseDecoder::default();
    assert!(decoder.push("data: {\"a\"").is_empty());
    let events = decoder.push(":1}\r\n\r\ndata: [DONE]\n\n");
    assert_eq!(events, vec!["{\"a\":1}".to_string(), "[DONE]".to_string()]);
}

#[test]
fn decoder_keeps_multiline_data_and_skips_comments() {
    let mut decoder = SseDecoder::default();
    let events = decoder.push(": ping\nevent: message\ndata: one\ndata: two\n\n");
    assert_eq!(events, vec!["one\ntwo".to_string()]);
}
