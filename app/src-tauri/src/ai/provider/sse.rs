#[derive(Default)]
pub(super) struct SseDecoder {
    buffer: Vec<u8>,
    data: String,
}

impl SseDecoder {
    pub(super) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(line) = self.take_line() {
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(std::mem::take(&mut self.data));
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
        let newline = self.buffer.iter().position(|&b| b == b'\n')?;
        let mut line = String::from_utf8_lossy(&self.buffer[..newline]).into_owned();
        self.buffer.drain(..=newline);
        if line.ends_with('\r') {
            line.pop();
        }
        Some(line)
    }
}

#[cfg(test)]
mod tests;
