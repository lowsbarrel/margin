use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{Context, Task, Window};
use margin_i18n::t_with;
use margin_workbench::{
    CurrentVault, OpenRequest, Platform, WorkbenchEvent, notify_error, run_blocking,
};

use crate::spotlight::Spotlight;

const OPEN: &str = "[[";
const CLOSE: &str = "]]";
const NOTE_EXTENSIONS: [&str; 2] = ["md", "canvas"];

pub(crate) fn link_citations(markdown: &str) -> String {
    let mut linked = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find(OPEN) {
        let after_open = &rest[start + OPEN.len()..];
        let Some(end) = after_open.find(CLOSE) else {
            break;
        };
        let inner = &after_open[..end];
        let is_embed = rest[..start].ends_with('!');
        linked.push_str(&rest[..start]);
        match citation_link(inner) {
            Some(link) if !is_embed => linked.push_str(&link),
            _ => {
                linked.push_str(OPEN);
                linked.push_str(inner);
                linked.push_str(CLOSE);
            }
        }
        rest = &after_open[end + CLOSE.len()..];
    }
    linked.push_str(rest);
    linked
}

fn citation_link(inner: &str) -> Option<String> {
    let (target, alias) = inner
        .split_once('|')
        .map_or((inner, None), |(target, alias)| (target, Some(alias)));
    let target = target.trim();
    if target.is_empty() || target.contains(['\n', '<', '>']) {
        return None;
    }
    let label = alias
        .map_or(target, str::trim)
        .replace('[', "\\[")
        .replace(']', "\\]");
    Some(format!("[{label}](<{target}>)"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkTarget<'url> {
    External(&'url str),
    Note {
        title: &'url str,
        heading: Option<&'url str>,
    },
}

pub(crate) fn classify(url: &str) -> LinkTarget<'_> {
    if url.contains("://") || url.starts_with("mailto:") {
        return LinkTarget::External(url);
    }
    let (title, heading) = url.split_once('#').map_or((url, None), |(title, heading)| {
        (title, Some(heading.trim()))
    });
    LinkTarget::Note {
        title: title.trim(),
        heading: heading.filter(|heading| !heading.is_empty()),
    }
}

pub(crate) fn open(url: &str, window: &mut Window, cx: &mut Context<'_, Spotlight>) -> Task<()> {
    let (title, heading) = match classify(url) {
        LinkTarget::External(target) => {
            if let Err(error) = Platform::open_external(cx, target) {
                notify_error(window, cx, "spotlight_link_open_failed", &error);
            }
            return Task::ready(());
        }
        LinkTarget::Note { title, heading } => (title.to_owned(), heading.map(str::to_owned)),
    };
    let Some(services) = CurrentVault::get(cx) else {
        return Task::ready(());
    };
    let names: Vec<String> = NOTE_EXTENSIONS
        .iter()
        .map(|extension| format!("{title}.{extension}"))
        .collect();
    let lookup = title.clone();
    let work = run_blocking(cx, move || {
        services
            .files
            .search_files(&lookup)
            .into_iter()
            .find(|entry| names.contains(&entry.name))
    });
    cx.spawn_in(window, async move |this, cx| {
        let found = work.await;
        drop(this.update_in(cx, |this, window, cx| {
            let Some(entry) = found else {
                let message = t_with(cx, "toast_note_not_found", &[("title", &title)]);
                window.push_notification(Notification::info(message), cx);
                return;
            };
            let request = OpenRequest::new(entry.path);
            let request = match heading {
                Some(heading) => request.at_anchor(heading),
                None => request,
            };
            cx.emit(WorkbenchEvent::Open(request));
            this.dismiss(cx);
        }));
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_links_become_markdown_links() {
        assert_eq!(
            link_citations("See [[Road map]] and [[Ideas#Later|later ideas]]."),
            "See [Road map](<Road map>) and [later ideas](<Ideas#Later>)."
        );
    }

    #[test]
    fn embeds_and_unclosed_links_stay_as_written() {
        assert_eq!(
            link_citations("![[photo.png]] [[open"),
            "![[photo.png]] [[open"
        );
    }

    #[test]
    fn labels_escape_brackets() {
        assert_eq!(link_citations("[[a]b]]"), "[a\\]b](<a]b>)");
    }

    #[test]
    fn classifies_clicked_links() {
        assert_eq!(
            classify("https://margin.app"),
            LinkTarget::External("https://margin.app")
        );
        assert_eq!(
            classify("Ideas#Later"),
            LinkTarget::Note {
                title: "Ideas",
                heading: Some("Later")
            }
        );
        assert_eq!(
            classify("Road map"),
            LinkTarget::Note {
                title: "Road map",
                heading: None
            }
        );
    }
}
