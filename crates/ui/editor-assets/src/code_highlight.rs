use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::Rope;
use gpui_kit::component::highlighter::{LanguageRegistry, SyntaxHighlighter};
use gpui_kit::component::input::HighlightStyleResolver;
use gpui_kit::{App, Global, HighlightStyle};
use margin_live_preview::SyntaxRole;

use crate::generations::Generations;

const CACHED_BLOCKS: usize = 256;

const ROLES: [SyntaxRole; 6] = [
    SyntaxRole::Keyword,
    SyntaxRole::String,
    SyntaxRole::Number,
    SyntaxRole::Function,
    SyntaxRole::Comment,
    SyntaxRole::Attribute,
];

const ALIASES: &[(&str, &str)] = &[
    ("sh", "bash"),
    ("shell", "bash"),
    ("zsh", "bash"),
    ("console", "bash"),
    ("h", "c"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("hpp", "cpp"),
    ("hh", "cpp"),
    ("hxx", "cpp"),
    ("c++", "cpp"),
    ("h++", "cpp"),
    ("scss", "css"),
    ("golang", "go"),
    ("xml", "html"),
    ("xhtml", "html"),
    ("svg", "html"),
    ("rss", "html"),
    ("plist", "html"),
    ("jsp", "java"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("node", "javascript"),
    ("ecmascript", "javascript"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("py", "python"),
    ("gyp", "python"),
    ("rs", "rust"),
    ("ts", "typescript"),
    ("mts", "typescript"),
    ("cts", "typescript"),
    ("yml", "yaml"),
];

type CodeSpans = Arc<[(Range<usize>, SyntaxRole)]>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CodeKey {
    grammar: &'static str,
    code_hash: u64,
    code_len: usize,
}

struct CodeHighlights {
    spans: Generations<CodeKey, CodeSpans>,
    highlighters: HashMap<&'static str, SyntaxHighlighter>,
}

impl Default for CodeHighlights {
    fn default() -> Self {
        Self {
            spans: Generations::with_capacity(CACHED_BLOCKS),
            highlighters: HashMap::new(),
        }
    }
}

impl Global for CodeHighlights {}

struct RoleResolver;

impl HighlightStyleResolver for RoleResolver {
    fn style(&self, name: &str) -> Option<HighlightStyle> {
        let role = syntax_role(name)?;
        let index = ROLES.iter().position(|candidate| *candidate == role)?;
        let encoded = u8::try_from(index).ok()?;
        Some(HighlightStyle {
            fade_out: Some(f32::from(encoded)),
            ..HighlightStyle::default()
        })
    }
}

fn decoded_role(style: &HighlightStyle) -> Option<SyntaxRole> {
    let encoded = style.fade_out?;
    ROLES
        .iter()
        .zip(0_u8..)
        .find(|(_, index)| (f32::from(*index) - encoded).abs() < 0.5)
        .map(|(role, _)| *role)
}

#[must_use]
pub fn highlight_code(language: &str, code: &str, cx: &mut App) -> CodeSpans {
    let Some(grammar) = grammar_name(language) else {
        return Arc::from([]);
    };
    if code.is_empty() {
        return Arc::from([]);
    }
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    let key = CodeKey {
        grammar,
        code_hash: hasher.finish(),
        code_len: code.len(),
    };
    let cache = cx.default_global::<CodeHighlights>();
    if let Some(spans) = cache.spans.get_mut(&key) {
        return Arc::clone(spans);
    }
    let highlighter = cache
        .highlighters
        .entry(grammar)
        .or_insert_with(|| SyntaxHighlighter::new(grammar));
    let spans = highlighted(highlighter, code);
    cache.spans.insert(key, Arc::clone(&spans));
    spans
}

fn highlighted(highlighter: &mut SyntaxHighlighter, code: &str) -> CodeSpans {
    highlighter.update(None, &Rope::new(), None);
    let rope = Rope::from(code);
    highlighter.update(None, &rope, None);
    highlighter
        .styles(&(0..code.len()), &RoleResolver)
        .into_iter()
        .filter_map(|(range, style)| decoded_role(&style).map(|role| (range, role)))
        .collect()
}

fn grammar_name(language: &str) -> Option<&'static str> {
    let lowered = language.trim().to_lowercase();
    let name = lowered
        .split(|character: char| character.is_whitespace() || matches!(character, ':' | '{'))
        .next()
        .filter(|name| !name.is_empty())?;
    let canonical = ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, canonical)| *canonical);
    let config = LanguageRegistry::singleton().language(canonical)?;
    if !config.has_grammar() {
        return None;
    }
    GRAMMARS
        .iter()
        .copied()
        .find(|grammar| *grammar == config.name.as_ref())
}

const GRAMMARS: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "python",
    "rust",
    "sql",
    "toml",
    "tsx",
    "typescript",
    "yaml",
];

fn syntax_role(capture: &str) -> Option<SyntaxRole> {
    match capture {
        "string.special.symbol" | "variable.special" | "variable.builtin" => {
            return Some(SyntaxRole::Number);
        }
        "punctuation.list_marker" | "punctuation.special" => return Some(SyntaxRole::Attribute),
        "function.builtin" | "type.builtin" | "tag.doctype" => return Some(SyntaxRole::Keyword),
        _ => {}
    }
    let head = capture.split('.').next().unwrap_or(capture);
    match head {
        "comment" => Some(SyntaxRole::Comment),
        "keyword" | "tag" | "include" | "conditional" | "repeat" | "exception" | "storageclass" => {
            Some(SyntaxRole::Keyword)
        }
        "string" | "character" | "escape" | "regex" => Some(SyntaxRole::String),
        "number" | "float" | "boolean" | "constant" | "preproc" | "label" => {
            Some(SyntaxRole::Number)
        }
        "function" | "method" | "type" | "constructor" | "title" | "enum" | "variant" => {
            Some(SyntaxRole::Function)
        }
        "attribute" | "property" | "field" | "parameter" => Some(SyntaxRole::Attribute),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_map_onto_the_six_syntax_roles() {
        assert_eq!(syntax_role("keyword.modifier"), Some(SyntaxRole::Keyword));
        assert_eq!(syntax_role("tag"), Some(SyntaxRole::Keyword));
        assert_eq!(syntax_role("string.escape"), Some(SyntaxRole::String));
        assert_eq!(syntax_role("boolean"), Some(SyntaxRole::Number));
        assert_eq!(syntax_role("constant.builtin"), Some(SyntaxRole::Number));
        assert_eq!(syntax_role("function.method"), Some(SyntaxRole::Function));
        assert_eq!(syntax_role("type"), Some(SyntaxRole::Function));
        assert_eq!(syntax_role("comment.doc"), Some(SyntaxRole::Comment));
        assert_eq!(syntax_role("property"), Some(SyntaxRole::Attribute));
        assert_eq!(syntax_role("function.builtin"), Some(SyntaxRole::Keyword));
        assert_eq!(syntax_role("variable"), None);
        assert_eq!(syntax_role("punctuation.bracket"), None);
    }

    #[test]
    fn every_role_survives_the_style_encoding() {
        for role in ROLES {
            let index = ROLES
                .iter()
                .position(|candidate| *candidate == role)
                .expect("role is listed");
            let style = HighlightStyle {
                fade_out: Some(f32::from(
                    u8::try_from(index).expect("six roles fit a byte"),
                )),
                ..HighlightStyle::default()
            };
            assert_eq!(decoded_role(&style), Some(role));
        }
        assert_eq!(decoded_role(&HighlightStyle::default()), None);
    }

    #[test]
    fn fence_info_resolves_through_aliases() {
        assert_eq!(grammar_name("TS"), Some("typescript"));
        assert_eq!(grammar_name("rust {.numberLines}"), Some("rust"));
        assert_eq!(grammar_name(" py:main.py"), Some("python"));
        assert_eq!(grammar_name("zsh"), Some("bash"));
        assert_eq!(grammar_name("brainfuck"), None);
        assert_eq!(grammar_name(""), None);
    }

    #[test]
    fn rust_code_gets_keyword_and_string_spans() {
        let mut highlighter = SyntaxHighlighter::new("rust");
        let code = "fn main() { let s = \"hi\"; }";
        let spans = highlighted(&mut highlighter, code);
        let role_of = |text: &str| {
            let start = code.find(text).expect("text is in the code");
            spans
                .iter()
                .find(|(range, _)| range.start <= start && start < range.end)
                .map(|(_, role)| *role)
        };
        assert_eq!(role_of("fn"), Some(SyntaxRole::Keyword));
        assert_eq!(role_of("\"hi\""), Some(SyntaxRole::String));
        assert_eq!(role_of("main"), Some(SyntaxRole::Function));
    }
}
