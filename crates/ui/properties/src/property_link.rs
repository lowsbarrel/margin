#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LinkTarget {
    WikiLink(String),
    Url(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PropertyLink {
    label: String,
    target: LinkTarget,
}

impl PropertyLink {
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) const fn target(&self) -> &LinkTarget {
        &self.target
    }
}

pub(crate) fn link_of(text: &str) -> Option<PropertyLink> {
    if let Some(inner) = text
        .strip_prefix("[[")
        .and_then(|rest| rest.strip_suffix("]]"))
        .filter(|inner| !inner.is_empty() && !inner.contains(']'))
    {
        let (target, alias) = inner.split_once('|').unwrap_or((inner, inner));
        return Some(PropertyLink {
            label: alias.trim().to_owned(),
            target: LinkTarget::WikiLink(target.trim().to_owned()),
        });
    }
    if let Some((label, href)) = markdown_link(text) {
        let label = if label.is_empty() { href } else { label };
        return Some(PropertyLink {
            label: label.to_owned(),
            target: LinkTarget::Url(href.to_owned()),
        });
    }
    is_web_url(text).then(|| PropertyLink {
        label: text.to_owned(),
        target: LinkTarget::Url(text.to_owned()),
    })
}

fn markdown_link(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('[')?.strip_suffix(')')?;
    let (label, href) = rest.split_once("](")?;
    if label.contains(']') {
        return None;
    }
    let href = href.strip_prefix('<').unwrap_or(href);
    let href = href.strip_suffix('>').unwrap_or(href).trim();
    (!href.is_empty() && !href.contains([')', '>'])).then_some((label, href))
}

fn is_web_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        !rest.is_empty()
            && !rest.contains(|character: char| {
                character.is_whitespace() || character == '<' || character == '>'
            })
    })
}

pub(crate) fn is_tag_name(text: &str) -> bool {
    let allowed =
        |character: char| character.is_alphanumeric() || matches!(character, '_' | '/' | '-');
    !text.is_empty()
        && text.chars().all(allowed)
        && text
            .chars()
            .any(|character| character.is_alphabetic() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_recognised_in_every_form() {
        let wiki = link_of("[[Roadmap|the plan]]").expect("a wiki link");
        assert_eq!(wiki.label, "the plan");
        assert_eq!(wiki.target, LinkTarget::WikiLink("Roadmap".to_owned()));
        let markdown = link_of("[](<Q3 plan.md>)").expect("a markdown link");
        assert_eq!(markdown.label, "Q3 plan.md");
        assert_eq!(markdown.target, LinkTarget::Url("Q3 plan.md".to_owned()));
        assert!(link_of("https://margin.app").is_some());
        assert_eq!(link_of("plain text"), None);
    }

    #[test]
    fn tag_names_need_a_letter() {
        assert!(is_tag_name("work/q3"));
        assert!(!is_tag_name("2024"));
        assert!(!is_tag_name("two words"));
    }
}
