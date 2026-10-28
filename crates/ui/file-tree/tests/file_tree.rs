use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::derive_vault;
use margin_file_tree::FileTree;
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
use margin_workbench::{CurrentVault, OpenRequest, SyncFactory, VaultServices, WorkbenchEvent};
use parking_lot::RwLock;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const HEADER_REMS: f32 = 46.0 / 14.0;
const ROW_REMS: f32 = 2.0;

struct NoSync;

impl SyncFactory for NoSync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        unreachable!("the file tree never configures sync")
    }
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

fn vault(files: &Arc<dyn FileSystem>) -> Arc<VaultServices> {
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let history = Arc::new(History::new(Arc::clone(files), Arc::clone(&clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
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
            Arc::clone(&clock),
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
            clock,
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    })
}

struct Fixture {
    files: Arc<dyn FileSystem>,
    window: WindowHandle<Root>,
    tree: Entity<FileTree>,
    events: Rc<RefCell<Vec<WorkbenchEvent>>>,
}

fn open_tree(cx: &TestAppContext, existing: &[&str]) -> Fixture {
    let files: Arc<dyn FileSystem> = Arc::new(MemoryFileSystem::new());
    for name in existing {
        files.write(&path(name), b"").expect("seed the vault");
    }
    let services = vault(&files);
    let (window, tree) = cx.update(|cx| {
        gpui_kit::init(cx);
        Translator::init(Some(Locale::English), cx);
        MarginTheme::init(Appearance::Light, cx);
        cx.set_global(CurrentVault(Some(services)));
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(320.0), px(640.0)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..WindowOptions::default()
        };
        let (window, tree) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| FileTree::new(window, cx))
        })
        .expect("open the test window");
        (window.downcast().expect("a Root window"), tree)
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    cx.update(|cx| {
        cx.subscribe(&tree, move |_, event: &WorkbenchEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.run_until_parked();
    Fixture {
        files,
        window,
        tree,
        events,
    }
}

impl Fixture {
    fn type_and_confirm(&self, text: &str, cx: &mut TestAppContext) {
        cx.update_window(self.window.into(), |_, window, cx| {
            window.input(text, cx);
            window.press("enter", cx);
        })
        .expect("the window is open");
        cx.run_until_parked();
    }

    fn new_note(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window.into(), |_, window, cx| {
            self.tree.update(cx, |tree, cx| tree.new_note(window, cx));
        })
        .expect("the window is open");
        cx.run_until_parked();
    }

    fn visual(&self, cx: &TestAppContext) -> VisualTestContext {
        VisualTestContext::from_window(self.window.into(), cx)
    }

    fn renamed(&self, from: &str, to: &str) -> bool {
        self.events.borrow().contains(&WorkbenchEvent::Renamed {
            from: path(from),
            to: path(to),
        })
    }
}

fn row_center(cx: &mut VisualTestContext, index: usize) -> Point<Pixels> {
    let rem = cx.update(|window, _| window.rem_size());
    let index = f32::from(u8::try_from(index).expect("a handful of rows"));
    let rows = ROW_REMS.mul_add(index, HEADER_REMS) + ROW_REMS / 2.0;
    point(px(120.0), rem * rows)
}

fn click_row(cx: &mut VisualTestContext, index: usize, modifiers: Modifiers) {
    let position = row_center(cx, index);
    cx.simulate_click(position, modifiers);
    cx.run_until_parked();
}

fn drag_row(cx: &mut VisualTestContext, from: usize, to: usize) {
    let (start, end) = (row_center(cx, from), row_center(cx, to));
    cx.simulate_mouse_move(start, None, Modifiers::none());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    for step in 1..=8_u8 {
        let t = f32::from(step) / 8.0;
        let position = point(start.x, start.y + (end.y - start.y) * t);
        cx.simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
    }
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_new_note_opens_without_starting_an_inline_rename(cx: &mut TestAppContext) {
    let fixture = open_tree(cx, &["Untitled.md"]);

    fixture.new_note(cx);

    let created = path("Untitled 1.md");
    assert!(fixture.files.exists(&created), "the note lands on disk");
    assert!(
        fixture
            .events
            .borrow()
            .contains(&WorkbenchEvent::Open(OpenRequest::new(created.clone()))),
        "creating a note opens it"
    );
    let selected = fixture.tree.read_with(cx, |tree, _| tree.selected_paths());
    assert_eq!(selected, [created]);
    fixture.type_and_confirm("Plan", cx);
    assert!(!fixture.files.exists(&path("Plan.md")), "no rename field");
}

#[gpui_kit::test]
fn f2_renames_the_focused_note_keeping_its_extension(cx: &mut TestAppContext) {
    let fixture = open_tree(cx, &["Docs/a.md", "Plan.md"]);
    let mut visual = fixture.visual(cx);

    click_row(&mut visual, 1, Modifiers::none());
    visual.simulate_keystrokes("f2");
    visual.run_until_parked();
    fixture.type_and_confirm("Roadmap", cx);

    assert!(fixture.renamed("Plan.md", "Roadmap.md"));
    assert!(fixture.files.exists(&path("Roadmap.md")));
    assert!(!fixture.files.exists(&path("Plan.md")));
}

#[gpui_kit::test]
fn an_invalid_name_keeps_the_note_where_it_is(cx: &mut TestAppContext) {
    let fixture = open_tree(cx, &["Plan.md"]);
    let mut visual = fixture.visual(cx);

    click_row(&mut visual, 0, Modifiers::none());
    visual.simulate_keystrokes("f2");
    visual.run_until_parked();
    fixture.type_and_confirm("a:b", cx);

    assert!(fixture.files.exists(&path("Plan.md")));
    assert!(
        !fixture
            .events
            .borrow()
            .iter()
            .any(|event| matches!(event, WorkbenchEvent::Renamed { .. })),
        "a rejected name renames nothing"
    );
}

#[gpui_kit::test]
fn dragging_a_note_onto_a_folder_moves_it_there(cx: &TestAppContext) {
    let fixture = open_tree(cx, &["Projects/Roadmap.md", "Welcome.md"]);
    let mut visual = fixture.visual(cx);

    drag_row(&mut visual, 1, 0);

    assert!(fixture.renamed("Welcome.md", "Projects/Welcome.md"));
    assert!(fixture.files.exists(&path("Projects/Welcome.md")));
    assert!(!fixture.files.exists(&path("Welcome.md")));
}

#[gpui_kit::test]
fn dragging_one_of_several_selected_notes_moves_the_whole_selection(cx: &TestAppContext) {
    let fixture = open_tree(cx, &["Archive/old.md", "a.md", "b.md", "c.md"]);
    let mut visual = fixture.visual(cx);

    click_row(&mut visual, 1, Modifiers::none());
    let toggle = visual.update(|_, cx| Modifiers::secondary_key(cx));
    click_row(&mut visual, 2, toggle);
    drag_row(&mut visual, 2, 0);

    assert!(fixture.files.exists(&path("Archive/a.md")));
    assert!(fixture.files.exists(&path("Archive/b.md")));
    assert!(fixture.files.exists(&path("c.md")), "unselected rows stay");
}

#[gpui_kit::test]
fn a_folder_never_moves_into_itself_and_a_note_dropped_on_a_sibling_stays(cx: &TestAppContext) {
    let fixture = open_tree(cx, &["Projects/Roadmap.md", "Welcome.md", "Zebra.md"]);
    let mut visual = fixture.visual(cx);

    click_row(&mut visual, 0, Modifiers::none());
    drag_row(&mut visual, 0, 1);
    drag_row(&mut visual, 2, 3);

    assert!(fixture.files.exists(&path("Projects/Roadmap.md")));
    assert!(fixture.files.exists(&path("Welcome.md")));
    assert!(
        !fixture
            .events
            .borrow()
            .iter()
            .any(|event| matches!(event, WorkbenchEvent::Renamed { .. })),
        "nothing moved"
    );
}
