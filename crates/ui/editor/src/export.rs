use gpui_kit::Context;
use margin_markdown::{HtmlOptions, escape_html, to_html};
use margin_workbench::{notify_error, notify_info, run_blocking};

use crate::note_editor::NoteEditor;
use crate::title::file_title;

const STYLE: &str = "body{max-width:750px;margin:0 auto;padding:40px;font-family:Inter,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:15px;line-height:1.6}\
h1,h2,h3,h4,h5,h6{font-weight:600;letter-spacing:-0.02em;line-height:1.3}\
.note-title{font-size:32px;font-weight:600;margin:0 0 1em}\
code,pre{font-family:'JetBrains Mono',monospace;font-size:0.875em}\
pre{padding:1em;border-radius:8px;overflow-x:auto;background:color-mix(in srgb,currentColor 6%,transparent)}\
blockquote{margin:0.5em 0;padding:0.25em 1em;border-left:2px solid color-mix(in srgb,currentColor 25%,transparent)}\
table{border-collapse:collapse;width:100%}th,td{border:1px solid color-mix(in srgb,currentColor 20%,transparent);padding:8px 12px;text-align:left}\
th{background:color-mix(in srgb,currentColor 6%,transparent)}img{max-width:100%;height:auto}\
hr{border:none;border-top:1px solid color-mix(in srgb,currentColor 20%,transparent)}\
@media print{body{padding:0}}";

pub(crate) fn document_html(title: &str, body: &str) -> String {
    let title = escape_html(title);
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<div class=\"note-title\">{title}</div>\n{body}</body>\n</html>\n"
    )
}

pub(crate) fn export_html(editor: &NoteEditor, cx: &Context<'_, NoteEditor>) {
    let title = file_title(editor.note_path());
    let html = document_html(
        &title,
        &to_html(editor.session().document(), HtmlOptions::new(false)),
    );
    let services = editor.services();
    let directory = services.root.clone();
    let prompt = cx.prompt_for_new_path(&directory, Some(&format!("{title}.html")));
    let handle = editor.window_handle();
    cx.spawn(async move |this, cx| {
        let Ok(Ok(Some(target))) = prompt.await else {
            return;
        };
        let written = cx.update(|cx| {
            run_blocking(cx, move || {
                services
                    .files
                    .write_external_file(&target, html.as_bytes())
                    .map_err(|error| error.to_string())
            })
        });
        let result = written.await;
        drop(handle.update(cx, |_, window, cx| {
            drop(this.update(cx, |_, cx| match result {
                Ok(()) => notify_info(window, cx, "editor_export_html_saved"),
                Err(error) => notify_error(window, cx, "toast_export_failed", &error),
            }));
        }));
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_the_body_with_an_escaped_title() {
        let html = document_html("A & B", "<p>x</p>\n");
        assert!(html.contains("<title>A &amp; B</title>"));
        assert!(html.contains("<p>x</p>"));
    }
}
