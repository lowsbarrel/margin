use crate::findings::Findings;
use crate::repository::Repository;

const LINE_COMMENT_FILES: &[&str] = &[".editorconfig", ".gitignore", ".gitattributes"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    Rust,
    Json,
    Toml,
    Yaml,
    LineComments,
}

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    for file in repository.files() {
        let Some(syntax) = syntax_of(file) else {
            continue;
        };
        let Some(text) = repository.read(file) else {
            continue;
        };
        let line = match syntax {
            Syntax::Rust | Syntax::Json => first_slash_comment(&text),
            Syntax::Toml => first_hash_comment(&text, HashRule::Anywhere),
            Syntax::Yaml => first_hash_comment(&text, HashRule::AfterWhitespace),
            Syntax::LineComments => first_full_line_comment(&text),
        };
        if let Some(line) = line {
            findings.add(
                file,
                format!("line {line}: no comments, anywhere (AGENTS.md)"),
            );
        }
    }
}

fn syntax_of(file: &str) -> Option<Syntax> {
    let name = file.rsplit('/').next().unwrap_or(file);
    if LINE_COMMENT_FILES.contains(&name) {
        return Some(Syntax::LineComments);
    }
    let extension = name.rsplit_once('.').map(|(_, extension)| extension)?;
    match extension {
        "rs" => Some(Syntax::Rust),
        "json" => Some(Syntax::Json),
        "toml" => Some(Syntax::Toml),
        "yml" | "yaml" => Some(Syntax::Yaml),
        _ => None,
    }
}

fn first_slash_comment(text: &str) -> Option<usize> {
    let characters: Vec<char> = text.chars().collect();
    let mut line = 1;
    let mut index = 0;
    while let Some(&current) = characters.get(index) {
        let next = characters.get(index + 1).copied();
        if current == '/' && matches!(next, Some('/' | '*')) {
            return Some(line);
        }
        let consumed = match current {
            '"' => skip_quoted(&characters, index, '"'),
            'r' if starts_raw_string(&characters, index) => skip_raw_string(&characters, index),
            '\'' => skip_character_literal(&characters, index),
            _ => index + 1,
        };
        line += characters.get(index..consumed).map_or(0, |skipped| {
            skipped
                .iter()
                .filter(|character| **character == '\n')
                .count()
        });
        index = consumed.max(index + 1);
    }
    None
}

fn skip_quoted(characters: &[char], start: usize, quote: char) -> usize {
    let mut index = start + 1;
    while let Some(&current) = characters.get(index) {
        if current == '\\' {
            index += 2;
            continue;
        }
        if current == quote {
            return index + 1;
        }
        index += 1;
    }
    index
}

fn starts_raw_string(characters: &[char], start: usize) -> bool {
    let preceded_by_identifier = start
        .checked_sub(1)
        .and_then(|previous| characters.get(previous))
        .is_some_and(|previous| previous.is_alphanumeric() || *previous == '_');
    if preceded_by_identifier && characters.get(start.wrapping_sub(1)) != Some(&'b') {
        return false;
    }
    let mut index = start + 1;
    while characters.get(index) == Some(&'#') {
        index += 1;
    }
    characters.get(index) == Some(&'"')
}

fn skip_raw_string(characters: &[char], start: usize) -> usize {
    let mut index = start + 1;
    let mut hashes = 0;
    while characters.get(index) == Some(&'#') {
        hashes += 1;
        index += 1;
    }
    index += 1;
    while let Some(&current) = characters.get(index) {
        let closes = current == '"'
            && (1..=hashes).all(|offset| characters.get(index + offset) == Some(&'#'));
        if closes {
            return index + 1 + hashes;
        }
        index += 1;
    }
    index
}

fn skip_character_literal(characters: &[char], start: usize) -> usize {
    match (characters.get(start + 1), characters.get(start + 2)) {
        (Some('\\'), _) => skip_quoted(characters, start, '\''),
        (Some(_), Some('\'')) => start + 3,
        _ => start + 1,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HashRule {
    Anywhere,
    AfterWhitespace,
}

fn first_hash_comment(text: &str, rule: HashRule) -> Option<usize> {
    for (number, line) in text.lines().enumerate() {
        let mut quote: Option<char> = None;
        let mut previous = ' ';
        for character in line.chars() {
            if let Some(open) = quote {
                if character == open {
                    quote = None;
                }
            } else if character == '"' || character == '\'' {
                quote = Some(character);
            } else if character == '#' && (rule == HashRule::Anywhere || previous.is_whitespace()) {
                return Some(number + 1);
            }
            previous = character;
        }
    }
    None
}

fn first_full_line_comment(text: &str) -> Option<usize> {
    text.lines()
        .position(|line| matches!(line.trim_start().chars().next(), Some('#' | ';')))
        .map(|index| index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_comments_outside_literals() {
        assert_eq!(first_slash_comment("let a = 1;\n// note\n"), Some(2));
        assert_eq!(first_slash_comment("/// doc\nfn a() {}"), Some(1));
        assert_eq!(first_slash_comment("let url = \"https://x\";\n"), None);
        assert_eq!(first_slash_comment("let raw = r#\"a // b\"#;\n"), None);
        assert_eq!(
            first_slash_comment("fn a<'a>(x: &'a str) -> char { '/' }\n"),
            None
        );
        assert_eq!(
            first_slash_comment("let s = \"\\\"//\";\nlet t = 1; /* x */"),
            Some(2)
        );
    }

    #[test]
    fn finds_hash_comments_outside_strings() {
        assert_eq!(
            first_hash_comment("a = \"#fff\"\n# note\n", HashRule::Anywhere),
            Some(2)
        );
        assert_eq!(
            first_hash_comment("run: echo ${#TITLE}\n", HashRule::AfterWhitespace),
            None
        );
        assert_eq!(
            first_hash_comment("key: value # note\n", HashRule::AfterWhitespace),
            Some(1)
        );
    }
}
