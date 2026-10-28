use std::cmp::Ordering;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PageBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PageBox {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    fn distance(&self, x: f32, y: f32) -> (f32, f32) {
        let gap = |value: f32, start: f32, size: f32| {
            if value < start {
                start - value
            } else {
                (value - start - size).max(0.0)
            }
        };
        (gap(y, self.y, self.height), gap(x, self.x, self.width))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PdfMatch {
    pub page: usize,
    pub boxes: Vec<PageBox>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct PageText {
    text: Vec<char>,
    boxes: Vec<Option<PageBox>>,
}

impl PageText {
    pub(crate) fn push(&mut self, character: char, bounds: PageBox) {
        if let Some(Some(previous)) = self.boxes.last() {
            let same_line = (previous.y - bounds.y).abs() < previous.height / 2.0;
            let gap = bounds.x - (previous.x + previous.width);
            if !character.is_whitespace() && (!same_line || gap > previous.height * 0.3) {
                self.text.push(if same_line { ' ' } else { '\n' });
                self.boxes.push(None);
            }
        }
        self.text.push(character);
        self.boxes.push(Some(bounds));
    }

    pub(crate) const fn len(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub(crate) fn find(&self, page: usize, needle: &str) -> Vec<PdfMatch> {
        let needle: Vec<char> = needle.trim().chars().map(fold).collect();
        let searchable: Vec<usize> = (0..self.text.len())
            .filter(|&index| self.text[index] != '\n' || self.boxes[index].is_some())
            .collect();
        if needle.is_empty() || needle.len() > searchable.len() {
            return Vec::new();
        }
        let haystack: Vec<char> = searchable
            .iter()
            .map(|&index| fold(self.text[index]))
            .collect();
        let mut found = Vec::new();
        let mut at = 0;
        while at + needle.len() <= haystack.len() {
            if haystack[at..at + needle.len()] == needle[..] {
                let last = searchable[at + needle.len() - 1];
                found.push(PdfMatch {
                    page,
                    boxes: self.highlight(searchable[at]..last + 1),
                });
                at += needle.len();
            } else {
                at += 1;
            }
        }
        found
    }

    #[must_use]
    pub(crate) fn highlight(&self, range: Range<usize>) -> Vec<PageBox> {
        merge_boxes(self.boxes.get(range).unwrap_or_default())
    }

    #[must_use]
    pub(crate) fn slice(&self, range: Range<usize>) -> String {
        self.text.get(range).unwrap_or_default().iter().collect()
    }

    #[must_use]
    pub(crate) fn is_over_text(&self, x: f32, y: f32) -> bool {
        self.boxes
            .iter()
            .flatten()
            .any(|bounds| bounds.contains(x, y))
    }

    #[must_use]
    pub(crate) fn caret_at(&self, x: f32, y: f32) -> usize {
        let nearest = self
            .boxes
            .iter()
            .enumerate()
            .filter_map(|(index, bounds)| bounds.map(|bounds| (index, bounds)))
            .min_by(|(_, a), (_, b)| {
                a.distance(x, y)
                    .partial_cmp(&b.distance(x, y))
                    .unwrap_or(Ordering::Equal)
            });
        nearest.map_or(0, |(index, bounds)| {
            if x > bounds.width.mul_add(0.5, bounds.x) {
                index + 1
            } else {
                index
            }
        })
    }

    #[must_use]
    pub(crate) fn word_at(&self, caret: usize) -> Range<usize> {
        if self.text.is_empty() {
            return 0..0;
        }
        let at = caret.min(self.text.len() - 1);
        let word = |character: &char| character.is_alphanumeric() || *character == '_';
        if !word(&self.text[at]) {
            return at..at + 1;
        }
        let start = self.text[..at]
            .iter()
            .rposition(|character| !word(character))
            .map_or(0, |index| index + 1);
        let end = self.text[at..]
            .iter()
            .position(|character| !word(character))
            .map_or(self.text.len(), |index| at + index);
        start..end
    }

    #[must_use]
    pub(crate) fn line_at(&self, caret: usize) -> Range<usize> {
        let at = caret.min(self.text.len());
        let start = self.text[..at]
            .iter()
            .rposition(|character| *character == '\n')
            .map_or(0, |index| index + 1);
        let end = self.text[at..]
            .iter()
            .position(|character| *character == '\n')
            .map_or(self.text.len(), |index| at + index);
        start..end
    }
}

fn fold(character: char) -> char {
    if character.is_whitespace() {
        return ' ';
    }
    character.to_lowercase().next().unwrap_or(character)
}

fn merge_boxes(boxes: &[Option<PageBox>]) -> Vec<PageBox> {
    let mut merged: Vec<PageBox> = Vec::new();
    for bounds in boxes.iter().flatten() {
        match merged.last_mut() {
            Some(last) if (last.y - bounds.y).abs() < last.height / 2.0 => {
                let right = (last.x + last.width).max(bounds.x + bounds.width);
                let bottom = (last.y + last.height).max(bounds.y + bounds.height);
                last.x = last.x.min(bounds.x);
                last.y = last.y.min(bounds.y);
                last.width = right - last.x;
                last.height = bottom - last.y;
            }
            _ => merged.push(*bounds),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(rows: &[&str]) -> PageText {
        let mut text = PageText::default();
        for (row, line) in rows.iter().enumerate() {
            for (index, character) in line.chars().enumerate() {
                #[expect(clippy::cast_precision_loss, reason = "small test indices")]
                let bounds = PageBox {
                    x: index as f32 * 10.0,
                    y: row as f32 * 20.0,
                    width: 10.0,
                    height: 12.0,
                };
                text.push(character, bounds);
            }
        }
        text
    }

    #[test]
    fn find_is_case_insensitive_matches_across_lines_and_merges_boxes() {
        let text = lines(&["Hello World", "again"]);
        let found = text.find(3, "world");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].page, 3);
        assert_eq!(found[0].boxes.len(), 1);
        assert!((found[0].boxes[0].x - 60.0).abs() < f32::EPSILON);
        assert!((found[0].boxes[0].width - 50.0).abs() < f32::EPSILON);
        assert!(text.find(0, "world again").is_empty());
        let wrapped = lines(&["final vaul", "t reference"]);
        assert_eq!(wrapped.find(0, "vault")[0].boxes.len(), 2);
        assert!(text.find(0, "  ").is_empty());
    }

    #[test]
    fn selection_helpers_follow_carets_words_and_lines() {
        let text = lines(&["Hello World", "again"]);
        assert_eq!(text.slice(0..text.len()), "Hello World\nagain");
        assert_eq!(text.caret_at(2.0, 5.0), 0);
        assert_eq!(text.caret_at(8.0, 5.0), 1);
        assert_eq!(text.caret_at(500.0, 5.0), 11);
        assert_eq!(text.caret_at(12.0, 23.0), 13);
        assert_eq!(text.slice(text.word_at(8)), "World");
        assert_eq!(text.slice(text.line_at(14)), "again");
        assert!(text.is_over_text(15.0, 25.0));
        assert!(!text.is_over_text(15.0, 15.0));
        assert_eq!(text.highlight(3..15).len(), 2);
    }
}
