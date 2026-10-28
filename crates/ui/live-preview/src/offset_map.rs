#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub display: usize,
    pub source: usize,
    pub len: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OffsetMap {
    pieces: Vec<Piece>,
    source_start: usize,
    source_end: usize,
}

impl OffsetMap {
    #[must_use]
    pub const fn new(source_start: usize, source_end: usize) -> Self {
        Self {
            pieces: Vec::new(),
            source_start,
            source_end,
        }
    }

    pub fn push(&mut self, display: usize, source: usize, len: usize) {
        if let Some(last) = self.pieces.last_mut()
            && last.display + last.len == display
            && last.source + last.len == source
        {
            last.len += len;
            return;
        }
        self.pieces.push(Piece {
            display,
            source,
            len,
        });
    }

    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    #[must_use]
    pub const fn source_start(&self) -> usize {
        self.source_start
    }

    #[must_use]
    pub const fn source_end(&self) -> usize {
        self.source_end
    }

    #[must_use]
    pub fn to_display(&self, source: usize) -> usize {
        let mut display = 0;
        for piece in &self.pieces {
            if source < piece.source {
                return display;
            }
            if source <= piece.source + piece.len {
                return piece.display + (source - piece.source);
            }
            display = piece.display + piece.len;
        }
        display
    }

    #[must_use]
    pub fn to_source(&self, display: usize) -> usize {
        let mut source = self.source_start;
        for piece in &self.pieces {
            if display < piece.display {
                return source;
            }
            if display <= piece.display + piece.len {
                return piece.source + (display - piece.display);
            }
            source = piece.source + piece.len;
        }
        source.max(self.source_start).min(self.source_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold_map() -> OffsetMap {
        let mut map = OffsetMap::new(10, 18);
        map.push(0, 12, 4);
        map
    }

    #[test]
    fn hidden_markers_collapse_to_the_visible_edges() {
        let map = bold_map();
        assert_eq!(map.to_display(10), 0);
        assert_eq!(map.to_display(14), 2);
        assert_eq!(map.to_display(17), 4);
        assert_eq!(map.to_source(0), 12);
        assert_eq!(map.to_source(4), 16);
    }

    #[test]
    fn adjacent_pieces_merge() {
        let mut map = OffsetMap::new(0, 6);
        map.push(0, 0, 3);
        map.push(3, 3, 3);
        assert_eq!(map.pieces().len(), 1);
    }
}
