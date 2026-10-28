use gpui_kit::{AppContext, Entity, TestAppContext};
use margin_editor_commands::SlashCommand;
use margin_editor_overlays::{Completion, CompletionMenu, Trigger, completion_trigger};
use margin_i18n::{Locale, Translator};
use margin_markdown::{Document, Span};

fn menu(cx: &mut TestAppContext) -> Entity<CompletionMenu> {
    cx.update(|cx| Translator::init(Some(Locale::English), cx));
    cx.new(CompletionMenu::new)
}

fn trigger_at_end(text: &str) -> Trigger {
    completion_trigger(&Document::parse(text), text.len()).expect("the text ends in a trigger")
}

#[gpui_kit::test]
fn slash_menu_filters_and_confirms_the_selected_command(cx: &mut TestAppContext) {
    let menu = menu(cx);
    menu.update(cx, |menu, cx| {
        menu.set_trigger(trigger_at_end("intro /zqxj"), cx);
    });
    assert_eq!(menu.read_with(cx, |menu, _| menu.confirm()), None);
    menu.update(cx, |menu, cx| {
        menu.set_trigger(trigger_at_end("intro /h2"), cx);
    });
    assert_eq!(
        menu.read_with(cx, |menu, _| menu.confirm()),
        Some(Completion::Slash {
            trigger: Span::new(6, 9),
            command: SlashCommand::Heading2,
        })
    );
}

#[gpui_kit::test]
fn selection_wraps_around_the_list(cx: &mut TestAppContext) {
    let menu = menu(cx);
    menu.update(cx, |menu, cx| menu.set_trigger(trigger_at_end("/"), cx));
    let first = menu.read_with(cx, |menu, _| menu.confirm());
    menu.update(cx, CompletionMenu::select_previous);
    assert_eq!(
        menu.read_with(cx, |menu, _| menu.confirm()),
        Some(Completion::Slash {
            trigger: Span::new(0, 1),
            command: SlashCommand::Date,
        })
    );
    menu.update(cx, CompletionMenu::select_next);
    assert_eq!(menu.read_with(cx, |menu, _| menu.confirm()), first);
}

#[gpui_kit::test]
fn vault_completions_are_empty_without_a_vault(cx: &mut TestAppContext) {
    let menu = menu(cx);
    menu.update(cx, |menu, cx| {
        menu.set_trigger(trigger_at_end("[[Road"), cx);
    });
    menu.read_with(cx, |menu, _| {
        assert!(menu.is_empty(), "no vault means no link candidates");
        assert!(!menu.is_pending(), "nothing is loading without a vault");
    });
}
