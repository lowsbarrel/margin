#[derive(Default)]
pub(super) struct SseDecoder {
    buffer: String,
    data: String,
}

impl SseDecoder {
    pub(super) fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buffer.push_str(chunk);
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
        let newline = self.buffer.find('\n')?;
        let mut line = self.buffer[..newline].to_string();
        self.buffer.drain(..=newline);
        if line.ends_with('\r') {
            line.pop();
        }
        Some(line)
    }
}

#[cfg(test)]
mod tests;
