use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    Action, AppContext, Bounds, Context, Entity, Focusable, InteractiveElement, IntoElement,
    Modifiers, MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled,
    TestAppContext, VisualTestContext, Window, WindowBounds, WindowHandle, WindowOptions, div,
    point, px, size,
};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::derive_vault;
use margin_editor::{NoteEditor, ViewMode};
use margin_editor_actions::{Cancel, Copy, Paste, SelectMatches, SelectRight};
use margin_files::FilesService;
use margin_history::History;
use margin_i18n::{Locale, Translator};
use margin_kernel::{Clock, FileSystem, HistoryStore, HostFiles, RelativePath};
use margin_search::SearchService;
use margin_settings::{AppSettings, S3Config, Settings};
use margin_sync::SyncService;
use margin_terminals::Terminals;
use margin_test_support::{
    FakePtyHost, FixedClock, MemoryFileSystem, MemoryHostFiles, MemorySearchIndex,
    MemoryVaultReader, ScriptedLanguageModel,
};
use margin_theme::{Appearance, MarginTheme};
use margin_trash::TrashService;
use margin_vaults::VaultService;
use margin_workbench::{
    AppServices, CurrentVault, DraggedFiles, FindInNote, OpenRequest, ReplaceInNote, SyncFactory,
    TabContent, VaultServices, WorkbenchEvent,
};
use parking_lot::RwLock;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

type Step<'step> = &'step dyn Fn(&mut Window, &mut gpui_kit::App);

struct NoSync;

impl SyncFactory for NoSync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        unreachable!("the editor never configures sync")
    }
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

fn vault(files: &Arc<dyn FileSystem>, clock: &Arc<dyn Clock>) -> Arc<VaultServices> {
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let history = Arc::new(History::new(Arc::clone(files), Arc::clone(clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(files),
        Arc::clone(&history_store),
        Arc::clone(clock),
    ));
    let root = PathBuf::from("vault");
    let derived = derive_vault(MNEMONIC).expect("a valid test mnemonic");
    Arc::new(VaultServices {
        root: root.clone(),
        name: "Notes".to_owned(),
        id: derived.id,
        key: derived.key,
        files: Arc::new(FilesService::new(
            Arc::clone(files),
            Arc::clone(&host),
            history_store,
            Arc::clone(&trash),
            root,
        )),
        attachments: Arc::new(AttachmentsService::new(
            Arc::clone(files),
            host,
            Arc::clone(&trash),
            Arc::clone(clock),
        )),
        trash,
        search: Arc::new(SearchService::new(
            Arc::new(MemorySearchIndex::new()),
            Arc::clone(files),
        )),
        history,
        settings: Arc::new(Settings::new(Arc::clone(files))),
        app_settings: Arc::new(RwLock::new(AppSettings {
            s3: None,
            attachment_folder: None,
            auto_sync: None,
            conflict_strategy: None,
            llm: None,
            locale: None,
            theme: None,
        })),
        sync: Arc::new(RwLock::new(None)),
        sync_factory: Arc::new(NoSync),
        ask: Arc::new(AskService::new(
            Arc::new(ScriptedLanguageModel::new(Vec::new())),
            Arc::new(MemoryVaultReader::new()),
            Arc::clone(clock),
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    })
}

struct Host {
    editor: Entity<NoteEditor>,
    dragged: Vec<RelativePath>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        let dragged = DraggedFiles::new(self.dragged.clone());
        div()
            .size_full()
            .relative()
            .child(self.editor.clone())
            .child(
                div()
                    .id("drag-source")
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(px(20.0))
                    .on_drag(dragged, |_, _, _, cx| cx.new(|_| Ghost)),
            )
    }
}

struct Ghost;

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div()
    }
}

struct Fixture {
    files: Arc<dyn FileSystem>,
    window: WindowHandle<Root>,
    editor: Entity<NoteEditor>,
    events: Rc<RefCell<Vec<WorkbenchEvent>>>,
}

fn open_note(cx: &mut TestAppContext, notes: &[(&str, &str)], open: &str) -> Fixture {
    let files: Arc<dyn FileSystem> = Arc::new(MemoryFileSystem::new());
    for (name, text) in notes {
        files
            .write(&path(name), text.as_bytes())
            .expect("seed the vault");
    }
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let services = vault(&files, &clock);
    let (window, editor) = cx.update(|cx| {
        gpui_kit::init(cx);
        Translator::init(Some(Locale::English), cx);
        MarginTheme::init(Appearance::Light, cx);
        cx.set_global(AppServices {
            vaults: Arc::new(VaultService::new(Arc::new(MemoryFileSystem::new()))),
            host_files: Arc::new(MemoryHostFiles::new()),
            clock,
            data_dir: PathBuf::from("data"),
        });
        cx.set_global(CurrentVault(Some(services)));
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        };
        let dragged = notes.iter().filter(|(name, _)| *name != open);
        let (note, dragged) = (path(open), dragged.map(|(name, _)| path(name)).collect());
        let (window, host) = gpui_kit::open_window(options, cx, |window, cx| {
            let editor = cx.new(|cx| NoteEditor::new(note, window, cx));
            cx.new(|_| Host { editor, dragged })
        })
        .expect("open the test window");
        let editor = host.read(cx).editor.clone();
        (window.downcast().expect("a Root window"), editor)
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(&editor, move |_, event: &WorkbenchEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.run_until_parked();
    let fixture = Fixture {
        files,
        window,
        editor,
        events,
    };
    fixture.render(cx);
    fixture
}

impl Fixture {
    fn render(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window.into(), |_, window, cx| window.render_frame(cx))
            .expect("the window is open");
        cx.run_until_parked();
    }

    fn file(&self, name: &str) -> String {
        String::from_utf8(self.files.read(&path(name)).expect("the note exists"))
            .expect("notes are UTF-8")
    }

    fn type_at(&self, offset: usize, text: &str, cx: &mut TestAppContext) {
        let editor = self.editor.clone();
        cx.update_window(self.window.into(), |_, window, cx| {
            editor.update(cx, |editor, cx| {
                editor.set_cursor_offset(offset, window, cx);
            });
            window.focus(&editor.read(cx).focus_handle(cx), cx);
            window.render_frame(cx);
            window.input(text, cx);
        })
        .expect("the window is open");
        cx.run_until_parked();
    }

    fn flush(&self, cx: &mut TestAppContext) {
        self.editor.update(cx, TabContent::flush);
        cx.run_until_parked();
    }

    fn act(&self, at: Option<usize>, action: &dyn Action, cx: &mut TestAppContext) -> bool {
        let editor = self.editor.clone();
        cx.update_window(self.window.into(), |_, window, cx| {
            if let Some(offset) = at {
                editor.update(cx, |editor, cx| {
                    editor.set_cursor_offset(offset, window, cx);
                });
            }
            window.focus(&editor.read(cx).focus_handle(cx), cx);
            window.render_frame(cx);
            window.dispatch_action(action.boxed_clone(), cx);
        })
        .expect("the window is open");
        cx.run_until_parked();
        cx.update_window(self.window.into(), |_, window, cx| {
            window.render_frame(cx);
            window.try_find("find-close").is_some()
        })
        .expect("the window is open")
    }
}

#[gpui_kit::test]
fn typing_then_flush_writes_the_note_and_reports_the_save(cx: &mut TestAppContext) {
    let fixture = open_note(cx, &[("a.md", "hello")], "a.md");

    fixture.type_at(5, " world", cx);
    assert!(fixture.editor.read_with(cx, |editor, _| editor.is_dirty()));
    fixture.flush(cx);

    assert_eq!(fixture.file("a.md"), "hello world");
    assert!(!fixture.editor.read_with(cx, |editor, _| editor.is_dirty()));
    assert!(
        fixture
            .events
            .borrow()
            .contains(&WorkbenchEvent::Saved(path("a.md")))
    );
}

#[gpui_kit::test]
fn switching_view_modes_keeps_the_text_identical(cx: &mut TestAppContext) {
    let source = "---\ntags: [a]\n---\n# Title\n\n**bold** and [[Target]]\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
    let fixture = open_note(cx, &[("a.md", source)], "a.md");

    for mode in [ViewMode::Raw, ViewMode::LivePreview, ViewMode::Raw] {
        fixture
            .editor
            .update(cx, |editor, cx| editor.set_view_mode(mode, cx));
        fixture.render(cx);
        assert_eq!(
            fixture.editor.read_with(cx, |editor, _| editor.view_mode()),
            mode
        );
        assert!(!fixture.editor.read_with(cx, |editor, _| editor.is_dirty()));
    }
    fixture.flush(cx);
    assert_eq!(fixture.file("a.md"), source);
}

#[gpui_kit::test]
fn clicking_a_wiki_link_opens_its_note(cx: &mut TestAppContext) {
    let fixture = open_note(
        cx,
        &[("a.md", "intro\n\n[[Target]]\n"), ("Target.md", "")],
        "a.md",
    );
    let opened = WorkbenchEvent::Open(OpenRequest::new(path("Target.md")));
    let mut visual = VisualTestContext::from_window(fixture.window.into(), cx);
    for step in 0..120_u16 {
        let y = px(f32::from(step) * 4.0);
        visual.simulate_click(point(px(70.0), y), Modifiers::none());
        visual.run_until_parked();
        if fixture.events.borrow().contains(&opened) {
            return;
        }
    }
    panic!("no click on the note body opened the wiki link");
}

#[gpui_kit::test]
fn replace_all_replaces_every_match(cx: &mut TestAppContext) {
    let fixture = open_note(cx, &[("a.md", "foo bar foo\nfoo")], "a.md");
    let editor = fixture.editor.clone();
    let steps: [Step<'_>; 3] = [
        &|window, cx| {
            window.focus(&editor.read(cx).focus_handle(cx), cx);
            window.render_frame(cx);
        },
        &|window, cx| window.dispatch_action(Box::new(ReplaceInNote), cx),
        &|window, cx| {
            window.render_frame(cx);
            window.input("foo", cx);
            window.render_frame(cx);
        },
    ];
    for step in steps {
        cx.update_window(fixture.window.into(), |_, window, cx| step(window, cx))
            .expect("the window is open");
        cx.run_until_parked();
    }
    let mut visual = VisualTestContext::from_window(fixture.window.into(), cx);
    let replace_all = visual.update(|window, _| window.find("find-replace-all").bounds());
    visual.simulate_click(
        point(replace_all.left() - px(120.0), replace_all.center().y),
        Modifiers::none(),
    );
    visual.simulate_input("baz");
    visual.simulate_click(replace_all.center(), Modifiers::none());
    visual.run_until_parked();
    fixture.flush(cx);
    assert_eq!(fixture.file("a.md"), "baz bar baz\nbaz");
}

#[gpui_kit::test]
fn typing_into_a_note_with_frontmatter_and_math_does_not_hang(cx: &mut TestAppContext) {
    let source = "---\ntags: [demo, smoke]\nstatus: draft\n---\n# Welcome\n\n$$\nx^2\n$$\n";
    let fixture = open_note(cx, &[("a.md", source)], "a.md");
    fixture.type_at(0, "XYZ", cx);
    fixture.flush(cx);
    assert_eq!(fixture.file("a.md"), format!("XYZ{source}"));
}

#[gpui_kit::test]
fn escape_collapses_cursors_then_hides_the_toolbar_then_closes_find(cx: &mut TestAppContext) {
    let fixture = open_note(cx, &[("a.md", "ab ab\n")], "a.md");
    fixture.act(Some(0), &SelectRight, cx);
    fixture.act(None, &SelectRight, cx);
    fixture.act(None, &SelectMatches, cx);
    assert!(fixture.act(None, &FindInNote, cx));
    assert!(fixture.act(None, &Cancel, cx), "cursors collapse first");
    assert!(fixture.act(None, &Cancel, cx), "then the toolbar hides");
    assert!(!fixture.act(None, &Cancel, cx), "then find closes");
}

#[gpui_kit::test]
fn copying_without_a_selection_pastes_the_whole_line_above(cx: &mut TestAppContext) {
    let fixture = open_note(cx, &[("a.md", "one\ntwo\n")], "a.md");
    fixture.act(Some(1), &Copy, cx);
    fixture.act(Some(5), &Paste, cx);
    fixture.flush(cx);
    assert_eq!(fixture.file("a.md"), "one\none\ntwo\n");
}

#[gpui_kit::test]
fn dropping_tree_notes_inserts_their_links_back_to_back(cx: &mut TestAppContext) {
    let notes = [("a.md", "intro"), ("Target.md", ""), ("Other.md", "")];
    let fixture = open_note(cx, &notes, "a.md");
    let mut visual = VisualTestContext::from_window(fixture.window.into(), cx);
    let source = point(px(790.0), px(10.0));
    let target = point(px(700.0), px(590.0));
    visual.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    visual.run_until_parked();
    fixture.flush(cx);
    assert_eq!(fixture.file("a.md"), "intro[[Target]][[Other]]");
}
