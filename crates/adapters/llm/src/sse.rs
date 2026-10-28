use std::mem;

#[derive(Default)]
pub(crate) struct SseDecoder {
    buffer: Vec<u8>,
    data: String,
}

impl SseDecoder {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(line) = self.take_line() {
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(mem::take(&mut self.data));
                }
            } else if let Some(rest) = line.strip_prefix("data:") {
                let value = rest.strip_prefix(' ').unwrap_or(rest);
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
        }
        events
    }

    fn take_line(&mut self) -> Option<String> {
        let newline = self.buffer.iter().position(|byte| *byte == b'\n')?;
        let mut line = String::from_utf8_lossy(&self.buffer[..newline]).into_owned();
        self.buffer.drain(..=newline);
        if line.ends_with('\r') {
            line.pop();
        }
        Some(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_survives_payloads_split_across_chunks() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(b"data: {\"a\"").is_empty());
        let events = decoder.push(b":1}\r\n\r\ndata: [DONE]\n\n");
        assert_eq!(events, vec!["{\"a\":1}".to_owned(), "[DONE]".to_owned()]);
    }

    #[test]
    fn decoder_keeps_multiline_data_and_skips_comments() {
        let mut decoder = SseDecoder::default();
        let events = decoder.push(b": ping\nevent: message\ndata: one\ndata: two\n\n");
        assert_eq!(events, vec!["one\ntwo".to_owned()]);
    }

    #[test]
    fn a_character_split_across_chunks_decodes_intact() {
        let mut decoder = SseDecoder::default();
        let payload = "data: {\"text\":\"caffè\"}\n\n";
        let bytes = payload.as_bytes();
        let split = payload.find('è').expect("the payload holds the character") + 1;
        assert!(decoder.push(&bytes[..split]).is_empty());
        let events = decoder.push(&bytes[split..]);
        assert_eq!(events, vec!["{\"text\":\"caffè\"}".to_owned()]);
    }
}
