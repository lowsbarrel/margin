use std::ops::RangeInclusive;
use std::rc::Rc;

use markup5ever_rcdom::Handle;

use crate::html_dom;

const DROPPED: [&str; 8] = [
    "style", "script", "meta", "title", "link", "noscript", "template", "head",
];
const CODE_VIEW_MARKERS: [&str; 9] = [
    "monaco-editor",
    "cm-editor",
    "CodeMirror",
    "blob-code",
    "blob-wrapper",
    "highlight-source",
    "linenumbers",
    "line-numbers",
    "hljs",
];
const MONOSPACE: [&str; 8] = [
    "monospace",
    "consolas",
    "courier",
    "menlo",
    "monaco",
    "source code",
    "jetbrains mono",
    "sf mono",
];
const WORD_BULLETS: [char; 7] = [
    '\u{b7}', '\u{2022}', 'o', '\u{a7}', '\u{25aa}', '-', '\u{2013}',
];

fn declarations(style: &str) -> impl Iterator<Item = (String, String)> + '_ {
    style.split(';').filter_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        Some((name.trim().to_lowercase(), value.trim().to_lowercase()))
    })
}

fn property(style: &str, name: &str) -> Option<String> {
    declarations(style).find_map(|(key, value)| (key == name).then_some(value))
}

fn hundreds(value: &str, digits: RangeInclusive<char>) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| digits.contains(&first))
        && characters.as_str().starts_with("00")
}

fn is_bold(style: &str) -> bool {
    property(style, "font-weight")
        .is_some_and(|value| value.starts_with("bold") || hundreds(&value, '6'..='9'))
}

fn is_plain_weight(style: &str) -> bool {
    property(style, "font-weight")
        .is_some_and(|value| value.starts_with("normal") || hundreds(&value, '1'..='4'))
}

fn is_italic(style: &str) -> bool {
    property(style, "font-style").is_some_and(|value| value.starts_with("italic"))
}

fn is_struck(style: &str) -> bool {
    declarations(style)
        .any(|(key, value)| key.starts_with("text-decoration") && value.contains("line-through"))
}

fn word_list_level(style: &str) -> Option<usize> {
    let value = property(style, "mso-list")?;
    let mut tokens = value.split_whitespace();
    let list = tokens.next()?.strip_prefix('l')?;
    if list.is_empty() || !list.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    tokens.next()?.strip_prefix("level")?.parse().ok()
}

fn word_marker(text: &str) -> Option<bool> {
    let marker = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{a0}');
    let mut characters = marker.chars();
    if let (Some(bullet), None) = (characters.next(), characters.next())
        && WORD_BULLETS.contains(&bullet)
    {
        return Some(false);
    }
    let label = marker.strip_suffix(['.', ')'])?;
    let count = label.chars().count();
    ((1..=4).contains(&count) && label.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then_some(true)
}

fn word_list_marker(paragraph: &Handle) -> Option<bool> {
    for span in html_dom::elements(paragraph)
        .iter()
        .filter(|element| html_dom::is(element, &["span"]))
    {
        if let Some(ordered) = word_marker(&html_dom::text_content(span)) {
            html_dom::remove(span);
            return Some(ordered);
        }
    }
    None
}

fn word_lists(body: &Handle) {
    let mut stack: Vec<Handle> = Vec::new();
    let paragraphs: Vec<Handle> = html_dom::elements(body)
        .into_iter()
        .filter(|element| html_dom::is(element, &["p"]))
        .collect();
    for paragraph in paragraphs {
        let Some(depth) = word_list_level(&html_dom::style(&paragraph)).filter(|depth| *depth > 0)
        else {
            continue;
        };
        let Some(ordered) = word_list_marker(&paragraph) else {
            continue;
        };
        let previous = html_dom::previous_element(&paragraph);
        let continues = matches!((&previous, stack.first()), (Some(previous), Some(first)) if Rc::ptr_eq(previous, first));
        if !continues {
            stack.clear();
        }
        stack.truncate(depth);
        while stack.len() < depth {
            let list = html_dom::create(if ordered { "ol" } else { "ul" });
            let owner = stack
                .last()
                .and_then(|list| html_dom::element_children(list).last().cloned());
            match owner {
                Some(owner) => html_dom::append(&owner, &list),
                None => html_dom::insert_before(&paragraph, &list),
            }
            stack.push(list);
        }
        let item = html_dom::create("li");
        html_dom::move_children(&paragraph, &item);
        if let Some(list) = stack.get(depth - 1) {
            html_dom::append(list, &item);
        }
        html_dom::remove(&paragraph);
    }
}

fn wrap_children(element: &Handle, name: &str) {
    let wrapper = html_dom::create(name);
    html_dom::move_children(element, &wrapper);
    html_dom::append(element, &wrapper);
}

fn styled_emphasis(body: &Handle) {
    for element in html_dom::elements(body) {
        if html_dom::is(&element, &["b", "strong"]) && is_plain_weight(&html_dom::style(&element)) {
            html_dom::unwrap(&element);
        }
    }
    let headed = [
        "h1", "h2", "h3", "h4", "h5", "h6", "th", "pre", "code", "b", "strong",
    ];
    for span in html_dom::elements(body) {
        if !html_dom::is(&span, &["span"]) || html_dom::attribute(&span, "style").is_none() {
            continue;
        }
        if html_dom::closest(&span, &headed) || html_dom::text_content(&span).trim().is_empty() {
            continue;
        }
        let style = html_dom::style(&span);
        if is_struck(&style) {
            wrap_children(&span, "del");
        }
        if is_italic(&style) {
            wrap_children(&span, "em");
        }
        if is_bold(&style) {
            wrap_children(&span, "strong");
        }
    }
}

pub(crate) fn prepare(document: &Handle) {
    for element in html_dom::elements(document) {
        if html_dom::is(&element, &DROPPED) {
            html_dom::remove(&element);
        }
    }
    for element in html_dom::elements(document) {
        let local = html_dom::attribute(&element, "href").is_some_and(|href| href.starts_with('#'));
        if html_dom::is(&element, &["a"]) && local {
            html_dom::unwrap(&element);
        }
    }
    let body = html_dom::find(document, "body").unwrap_or_else(|| Rc::clone(document));
    word_lists(&body);
    styled_emphasis(&body);
}

fn monospace(style: &str) -> bool {
    let style = style.to_lowercase();
    MONOSPACE.iter().any(|font| style.contains(font))
}

pub(crate) fn is_code_editor_html(html: &str, document: &Handle) -> bool {
    let Some(body) = html_dom::find(document, "body") else {
        return false;
    };
    let first = html_dom::element_children(&body).into_iter().next();
    let preformatted = first.is_some_and(|root| {
        property(&html_dom::style(&root), "white-space")
            .is_some_and(|value| value.starts_with("pre"))
    });
    if preformatted || CODE_VIEW_MARKERS.iter().any(|marker| html.contains(marker)) {
        return true;
    }
    let runs: Vec<Handle> = html_dom::elements(&body)
        .into_iter()
        .filter(|element| {
            html_dom::element_children(element).is_empty()
                && !html_dom::text_content(element).trim().is_empty()
        })
        .collect();
    !runs.is_empty()
        && runs.iter().all(|run| {
            monospace(&html_dom::style(run))
                || html_dom::parent(run).is_some_and(|parent| monospace(&html_dom::style(&parent)))
        })
}
