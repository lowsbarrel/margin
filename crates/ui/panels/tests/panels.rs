use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::test_support;
use gpui_kit::component::Root;
use gpui_kit::{
    AnyView, App, AppContext, Context, ElementId, Entity, EventEmitter, Focusable, IntoElement,
    Modifiers, Render, TestAppContext, VisualTestContext, Window, div,
};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::derive_vault;
use margin_files::FilesService;
use margin_history::History;
use margin_i18n::{Locale, Translator};
use margin_kernel::{Clock, FileSystem, HistoryStore, HostFiles, PageRequest, RelativePath};
use margin_panels::{HistoryPanel, open_trash_dialog};
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
use margin_workbench::{AppServices, CurrentVault, SyncFactory, VaultServices, WorkbenchEvent};
use parking_lot::RwLock;

const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                        abandon abandon about";

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div()
    }
}

struct NoSync;

impl SyncFactory for NoSync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        unreachable!("the panel tests never configure sync")
    }
}

struct Fixture {
    files: Arc<MemoryFileSystem>,
    services: Arc<VaultServices>,
    clock: Arc<dyn Clock>,
}

fn fixture() -> Fixture {
    let vault = derive_vault(MNEMONIC).expect("the test mnemonic is valid");
    let root = PathBuf::from("/vault");
    let memory = Arc::new(MemoryFileSystem::new());
    let files: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(&memory);
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let history = Arc::new(History::new(Arc::clone(&files), Arc::clone(&clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(&files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
    ));
    let services = VaultServices {
        root: root.clone(),
        name: "Notes".to_owned(),
        id: vault.id,
        key: vault.key,
        files: Arc::new(FilesService::new(
            Arc::clone(&files),
            Arc::clone(&host),
            history_store,
            Arc::clone(&trash),
            root,
        )),
        trash: Arc::clone(&trash),
        attachments: Arc::new(AttachmentsService::new(
            Arc::clone(&files),
            host,
            trash,
            Arc::clone(&clock),
        )),
        search: Arc::new(SearchService::new(
            Arc::new(MemorySearchIndex::new()),
            Arc::clone(&files),
        )),
        history,
        settings: Arc::new(Settings::new(Arc::clone(&files))),
        app_settings: Arc::new(RwLock::new(AppSettings::default())),
        sync: Arc::new(RwLock::new(None)),
        sync_factory: Arc::new(NoSync),
        ask: Arc::new(AskService::new(
            Arc::new(ScriptedLanguageModel::new(Vec::new())),
            Arc::new(MemoryVaultReader::new()),
            Arc::clone(&clock),
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    };
    Fixture {
        files: memory,
        services: Arc::new(services),
        clock,
    }
}

fn open_window<'window>(
    fixture: &Fixture,
    cx: &'window mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut App) -> AnyView,
) -> &'window mut VisualTestContext {
    let services = Arc::clone(&fixture.services);
    let clock = Arc::clone(&fixture.clock);
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Translator::init(Some(Locale::English), cx);
        MarginTheme::init(Appearance::Light, cx);
        cx.set_global(AppServices {
            vaults: Arc::new(VaultService::new(Arc::new(MemoryFileSystem::new()))),
            host_files: Arc::new(MemoryHostFiles::new()),
            clock,
            data_dir: PathBuf::from("/data"),
        });
        cx.set_global(CurrentVault(Some(services)));
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = build(window, cx);
        Root::new(view, window, cx)
    });
    cx
}

fn click(cx: &mut VisualTestContext, id: &ElementId) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let target =
        cx.update(|window, _| test_support::find(window, &[], id).expect("the control is painted"));
    cx.simulate_click(target.bounds().center(), Modifiers::none());
    cx.run_until_parked();
}

fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_keystrokes(keystrokes);
    cx.run_until_parked();
}

#[cfg(target_os = "macos")]
const SECONDARY: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const SECONDARY: &str = "ctrl";

fn trash_paths(fixture: &Fixture) -> Vec<RelativePath> {
    fixture
        .services
        .trash
        .list(&PageRequest::first(usize::MAX))
        .items
        .into_iter()
        .map(|item| item.path)
        .collect()
}

fn trash(fixture: &Fixture, raw: &str) {
    fixture.files.insert(raw, b"kept");
    fixture
        .services
        .trash
        .move_entry(&path(raw))
        .expect("the note moves to the trash");
}

fn open_trash<'window>(
    fixture: &Fixture,
    cx: &'window mut TestAppContext,
) -> (Events, &'window mut VisualTestContext) {
    let cx = open_window(fixture, cx, |_, cx| cx.new(|_| Host).into());
    let events = Events::default();
    let recorded = Rc::clone(&events);
    cx.update(|window, cx| {
        open_trash_dialog(window, cx, move |event, _, _| {
            recorded.borrow_mut().push(event);
        });
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    (events, cx)
}

type Events = Rc<RefCell<Vec<WorkbenchEvent>>>;

fn mount<'window, V: Render + EventEmitter<WorkbenchEvent>>(
    fixture: &Fixture,
    cx: &'window mut TestAppContext,
    build: fn(&mut Window, &mut Context<'_, V>) -> V,
) -> (Entity<V>, Events, &'window mut VisualTestContext) {
    let events = Events::default();
    let mounted = Rc::new(RefCell::new(None));
    let recorded = Rc::clone(&events);
    let slot = Rc::clone(&mounted);
    let cx = open_window(fixture, cx, move |window, cx| {
        let view = cx.new(|cx| build(window, cx));
        cx.subscribe(&view, move |_, event: &WorkbenchEvent, _| {
            recorded.borrow_mut().push(event.clone());
        })
        .detach();
        slot.replace(Some(view.clone()));
        view.into()
    });
    let view = mounted.take().expect("the view was built");
    (view, events, cx)
}

fn history_with_version<'window>(
    fixture: &Fixture,
    cx: &'window mut TestAppContext,
) -> (Entity<HistoryPanel>, Events, &'window mut VisualTestContext) {
    let note = path("notes/plan.md");
    fixture
        .services
        .history
        .save(&note, b"first draft")
        .expect("the snapshot is saved");
    fixture.files.insert("notes/plan.md", b"rewritten");
    let (panel, events, cx) = mount(fixture, cx, HistoryPanel::new);
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.set_note(Some(note), cx)));
    cx.run_until_parked();
    (panel, events, cx)
}

fn restored_note(fixture: &Fixture) -> Vec<u8> {
    fixture
        .services
        .files
        .read_file(&path("notes/plan.md"))
        .expect("the note exists")
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

#[gpui_kit::test]
fn restoring_a_version_writes_the_note_and_reports_it_saved(cx: &mut TestAppContext) {
    let fixture = fixture();
    let (_, events, cx) = history_with_version(&fixture, cx);

    click(cx, &ElementId::from(("history-restore", 0_usize)));

    assert_eq!(restored_note(&fixture), b"first draft");
    assert_eq!(
        events.borrow().as_slice(),
        [WorkbenchEvent::Saved(path("notes/plan.md"))]
    );
    let versions = fixture
        .services
        .history
        .list(&path("notes/plan.md"))
        .expect("the history lists");
    assert_eq!(versions.len(), 2, "the rewritten text is kept as a version");
}

#[gpui_kit::test]
fn restoring_from_the_trash_reveals_the_restored_item(cx: &mut TestAppContext) {
    let fixture = fixture();
    trash(&fixture, "notes/old.md");
    let (events, cx) = open_trash(&fixture, cx);

    click(cx, &ElementId::from(("trash-restore", 0_usize)));

    assert_eq!(
        events.borrow().as_slice(),
        [WorkbenchEvent::Reveal(path("notes/old.md"))]
    );
    assert!(fixture.services.files.exists(&path("notes/old.md")));
    assert!(trash_paths(&fixture).is_empty());
}

#[gpui_kit::test]
fn keyboard_selects_and_restores_a_version(cx: &mut TestAppContext) {
    let fixture = fixture();
    let (panel, events, cx) = history_with_version(&fixture, cx);
    cx.update(|window, cx| panel.read(cx).focus_handle(cx).focus(window, cx));

    keys(cx, "down");
    keys(cx, &format!("{SECONDARY}-enter"));

    assert_eq!(restored_note(&fixture), b"first draft");
    assert_eq!(
        events.borrow().as_slice(),
        [WorkbenchEvent::Saved(path("notes/plan.md"))]
    );
}
