use margin_markdown::{Document, HtmlOptions, to_html};

fn html(source: &str) -> String {
    to_html(&Document::parse(source), HtmlOptions::default())
}

fn raw_html(source: &str) -> String {
    to_html(&Document::parse(source), HtmlOptions::new(true))
}

#[test]
fn headings_and_paragraphs() {
    assert_eq!(html("# Hi\n"), "<h1>Hi</h1>\n");
    assert_eq!(html("### Hi\n"), "<h3>Hi</h3>\n");
    assert_eq!(html("Hi\n===\n"), "<h1>Hi</h1>\n");
    assert_eq!(html("Hello *world*\n"), "<p>Hello <em>world</em></p>\n");
    assert_eq!(html("a\nb\n"), "<p>a\nb</p>\n");
    assert_eq!(html("a  \nb\n"), "<p>a<br>\nb</p>\n");
}

#[test]
fn inline_formatting() {
    assert_eq!(html("**b**\n"), "<p><strong>b</strong></p>\n");
    assert_eq!(html("~~s~~\n"), "<p><s>s</s></p>\n");
    assert_eq!(html("==h==\n"), "<p><mark>h</mark></p>\n");
    assert_eq!(html("`c`\n"), "<p><code>c</code></p>\n");
    assert_eq!(html("\\*x\\*\n"), "<p>*x*</p>\n");
}

#[test]
fn links_and_images() {
    assert_eq!(html("[a](u)\n"), "<p><a href=\"u\">a</a></p>\n");
    assert_eq!(
        html("[a](u \"t\")\n"),
        "<p><a href=\"u\" title=\"t\">a</a></p>\n"
    );
    assert_eq!(
        html("![alt](s.png)\n"),
        "<p><img src=\"s.png\" alt=\"alt\"></p>\n"
    );
    assert_eq!(
        html("<https://x>\n"),
        "<p><a href=\"https://x\">https://x</a></p>\n"
    );
    assert_eq!(
        html("see www.x.com\n"),
        "<p>see <a href=\"http://www.x.com\">www.x.com</a></p>\n"
    );
    assert_eq!(
        html("me@x.com\n"),
        "<p><a href=\"mailto:me@x.com\">me@x.com</a></p>\n"
    );
}

#[test]
fn obsidian_constructs() {
    assert_eq!(
        html("[[N|A]]\n"),
        "<p><a class=\"wiki-link\" href=\"#\">A</a></p>\n"
    );
    assert_eq!(
        html("![[i.png]]\n"),
        "<p><img class=\"wiki-embed\" src=\"i.png\" alt=\"i.png\"></p>\n"
    );
    assert_eq!(html("#tag\n"), "<p>#tag</p>\n");
    assert_eq!(
        html("$x$\n"),
        "<p><span class=\"math-inline\">x</span></p>\n"
    );
    assert_eq!(html("$$x$$\n"), "<div class=\"math-block\">x</div>\n");
    assert_eq!(html("%%hidden%%\n"), "<p></p>\n");
}

#[test]
fn lists_and_tasks() {
    assert_eq!(
        html("- a\n- b\n"),
        "<ul>\n<li>a\n</li>\n<li>b\n</li>\n</ul>\n"
    );
    assert_eq!(html("1. a\n"), "<ol>\n<li>a\n</li>\n</ol>\n");
    assert_eq!(html("3. a\n"), "<ol start=\"3\">\n<li>a\n</li>\n</ol>\n");
    assert_eq!(
        html("- [x] done\n- [ ] todo\n"),
        "<ul class=\"task-list\">\n<li class=\"task-item\"><input class=\"task-checkbox\" type=\"checkbox\" disabled checked>done\n</li>\n<li class=\"task-item\"><input class=\"task-checkbox\" type=\"checkbox\" disabled>todo\n</li>\n</ul>\n"
    );
}

#[test]
fn loose_lists_keep_paragraphs() {
    assert_eq!(
        html("- a\n\n- b\n"),
        "<ul>\n<li><p>a</p>\n</li>\n<li><p>b</p>\n</li>\n</ul>\n"
    );
}

#[test]
fn quotes_callouts_and_rules() {
    assert_eq!(html("> q\n"), "<blockquote>\n<p>q</p>\n</blockquote>\n");
    assert_eq!(
        html("> [!note] T\n> body\n"),
        "<blockquote class=\"callout callout-note\">\n<p class=\"callout-title\">T</p>\n<p>body</p>\n</blockquote>\n"
    );
    assert_eq!(
        html(":::warning\nbody\n:::\n"),
        "<div class=\"callout callout-warning\">\n<p class=\"callout-title\">Warning</p>\n<p>body</p>\n</div>\n"
    );
    assert_eq!(html("---\n"), "<hr>\n");
}

#[test]
fn code_blocks() {
    assert_eq!(
        html("```rust\nlet x = 1;\n```\n"),
        "<pre><code class=\"language-rust\">let x = 1;</code></pre>\n"
    );
    assert_eq!(html("```\nplain\n```\n"), "<pre><code>plain</code></pre>\n");
    assert_eq!(
        html("```mermaid\ngraph TD\n```\n"),
        "<div data-type=\"mermaid\" data-mermaid=\"graph TD\"></div>\n"
    );
    assert_eq!(html("    indented\n"), "<pre><code>indented</code></pre>\n");
    assert_eq!(
        html("```\n<b>\n```\n"),
        "<pre><code>&lt;b&gt;</code></pre>\n"
    );
}

#[test]
fn tables_render_alignment() {
    let source = "| a | b |\n| :-- | --- |\n| 1 | 2 |\n";
    assert_eq!(
        html(source),
        "<table>\n<thead>\n<tr><th style=\"text-align:left\">a</th><th>b</th></tr>\n</thead>\n<tbody>\n<tr><td style=\"text-align:left\">1</td><td>2</td></tr>\n</tbody>\n</table>\n"
    );
}

#[test]
fn footnotes_render_and_number() {
    let source = "see[^a]\n\n[^a]: body\n";
    assert_eq!(
        html(source),
        "<p>see<sup class=\"footnote-ref\"><a href=\"#fn-1\" id=\"fnref-1\">1</a></sup></p>\n<div class=\"footnote-def\" id=\"fn-1\">\n<p>body</p>\n</div>\n"
    );
}

#[test]
fn frontmatter_and_comments_are_dropped() {
    assert_eq!(html("---\nt: 1\n---\nbody\n"), "<p>body</p>\n");
    assert_eq!(html("%%\nhidden\n%%\n"), "");
}

#[test]
fn raw_html_is_escaped_unless_requested() {
    assert_eq!(html("<div>raw</div>\n"), "&lt;div&gt;raw&lt;/div&gt;\n");
    assert_eq!(raw_html("<div>raw</div>\n"), "<div>raw</div>\n");
    assert_eq!(html("a <b>x</b> c\n"), "<p>a &lt;b&gt;x&lt;/b&gt; c</p>\n");
    assert_eq!(raw_html("a <b>x</b> c\n"), "<p>a <b>x</b> c</p>\n");
}

#[test]
fn escapes_are_encoded_in_attributes() {
    assert_eq!(
        html("[a](u?a=1&b=\"2\")\n"),
        "<p><a href=\"u?a=1&amp;b=&quot;2&quot;\">a</a></p>\n"
    );
}
