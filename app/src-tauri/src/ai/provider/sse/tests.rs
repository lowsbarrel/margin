use super::*;

#[test]
fn decoder_survives_payloads_split_across_chunks() {
    let mut decoder = SseDecoder::default();
    assert!(decoder.push(b"data: {\"a\"").is_empty());
    let events = decoder.push(b":1}\r\n\r\ndata: [DONE]\n\n");
    assert_eq!(events, vec!["{\"a\":1}".to_string(), "[DONE]".to_string()]);
}

#[test]
fn decoder_keeps_multiline_data_and_skips_comments() {
    let mut decoder = SseDecoder::default();
    let events = decoder.push(b": ping\nevent: message\ndata: one\ndata: two\n\n");
    assert_eq!(events, vec!["one\ntwo".to_string()]);
}

#[test]
fn a_character_split_across_chunks_decodes_intact() {
    let mut decoder = SseDecoder::default();
    let payload = "data: {\"text\":\"caffè\"}\n\n";
    let bytes = payload.as_bytes();
    // Split inside the first byte pair of 'è', where a network chunk boundary can land.
    let split = payload.find('è').unwrap() + 1;
    assert!(decoder.push(&bytes[..split]).is_empty());
    let events = decoder.push(&bytes[split..]);
    assert_eq!(events, vec!["{\"text\":\"caffè\"}".to_string()]);
}
