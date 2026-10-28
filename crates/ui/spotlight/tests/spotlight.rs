use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions, px,
    size,
};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::{DerivedVault, derive_vault};
use margin_files::FilesService;
use margin_history::History;
use margin_i18n::{Locale, Translator};
use margin_kernel::{Clock, FileSystem, HistoryStore, HostFiles, RelativePath};
use margin_search::SearchService;
use margin_settings::{AppSettings, S3Config, Settings};
use margin_spotlight::{Spotlight, SpotlightEvent, SpotlightMode};
use margin_sync::SyncService;
use margin_terminals::Terminals;
use margin_test_support::{
    FakePtyHost, FixedClock, InstantTimer, MemoryBlobStore, MemoryFileSystem, MemoryHostFiles,
    MemorySearchIndex, MemorySnapshotStore, MemoryVaultReader, ScriptedLanguageModel,
};
use margin_theme::{Appearance, MarginTheme};
use margin_trash::TrashService;
use margin_workbench::{
    CurrentVault, OpenRequest, OpenSpotlight, OpenTarget, ReplaceInNote, SyncFactory,
    VaultServices, WorkbenchEvent,
};
use parking_lot::RwLock;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

struct MemorySync {
    files: Arc<dyn FileSystem>,
    clock: Arc<dyn Clock>,
    vault: DerivedVault,
}

impl SyncFactory for MemorySync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        Arc::new(SyncService::new(
            Arc::clone(&self.files),
            Arc::new(MemoryBlobStore::new()),
            Arc::clone(&self.clock),
            Arc::new(InstantTimer::new()),
            Arc::new(MemorySnapshotStore::new()),
            self.vault.clone(),
        ))
    }
}

fn vault_services(disk: &Arc<MemoryFileSystem>) -> Arc<VaultServices> {
    let files: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(disk);
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let history = Arc::new(History::new(Arc::clone(&files), Arc::clone(&clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(&files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
    ));
    let search = Arc::new(SearchService::new(
        Arc::new(MemorySearchIndex::new()),
        Arc::clone(&files),
    ));
    search.rebuild().expect("the memory index builds");
    let vault = derive_vault(MNEMONIC).expect("the test mnemonic is valid");
    Arc::new(VaultServices {
        root: PathBuf::from("vault"),
        name: "vault".to_owned(),
        id: vault.id.clone(),
        key: vault.key,
        files: Arc::new(FilesService::new(
            Arc::clone(&files),
            Arc::clone(&host),
            history_store,
            Arc::clone(&trash),
            PathBuf::from("vault"),
        )),
        attachments: Arc::new(AttachmentsService::new(
            Arc::clone(&files),
            host,
            Arc::clone(&trash),
            Arc::clone(&clock),
        )),
        trash,
        search,
        history,
        settings: Arc::new(Settings::new(Arc::clone(&files))),
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
        sync_factory: Arc::new(MemorySync {
            files: Arc::clone(&files),
            clock: Arc::clone(&clock),
            vault,
        }),
        ask: Arc::new(AskService::new(
            Arc::new(ScriptedLanguageModel::new(Vec::new())),
            Arc::new(MemoryVaultReader::new()),
            clock,
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    })
}

struct Harness {
    window: AnyWindowHandle,
    spotlight: Entity<Spotlight>,
    events: Rc<RefCell<Vec<WorkbenchEvent>>>,
    dismissed: Rc<RefCell<usize>>,
}

impl Harness {
    fn open(disk: &Arc<MemoryFileSystem>, mode: SpotlightMode, cx: &mut TestAppContext) -> Self {
        let services = vault_services(disk);
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
            MarginTheme::init(Appearance::Light, cx);
            Translator::init(Some(Locale::English), cx);
            cx.set_global(CurrentVault(Some(services)));
        });
        let (window, spotlight) = cx.update(|cx| {
            let bounds = Bounds::centered(None, size(px(900.0), px(700.0)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..WindowOptions::default()
                },
                cx,
                |window, cx| cx.new(|cx| Spotlight::new(window, cx)),
            )
            .expect("the test window opens")
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let dismissed = Rc::new(RefCell::new(0));
        cx.update(|cx| {
            let recorded = Rc::clone(&events);
            cx.subscribe(&spotlight, move |_, event: &WorkbenchEvent, _| {
                recorded.borrow_mut().push(event.clone());
            })
            .detach();
            let closed = Rc::clone(&dismissed);
            cx.subscribe(&spotlight, move |_, _: &SpotlightEvent, _| {
                *closed.borrow_mut() += 1;
            })
            .detach();
        });
        let harness = Self {
            window,
            spotlight,
            events,
            dismissed,
        };
        let spotlight = harness.spotlight.clone();
        cx.update_window(window, |_, window, cx| {
            spotlight.update(cx, |spotlight, cx| spotlight.open(mode, window, cx));
            window.render_frame(cx);
        })
        .expect("the window is open");
        harness
    }

    fn type_text(&self, text: &str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| window.input(text, cx))
            .expect("the window is open");
        self.settle(cx);
    }

    fn press(&self, key: &str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| window.press(key, cx))
            .expect("the window is open");
        self.settle(cx);
    }

    fn settle(&self, cx: &mut TestAppContext) {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update_window(self.window, |_, window, cx| window.render_frame(cx))
            .expect("the window is open");
    }

    fn events(&self) -> Vec<WorkbenchEvent> {
        self.events.borrow().clone()
    }
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid test path")
}

fn vault() -> Arc<MemoryFileSystem> {
    let disk = Arc::new(MemoryFileSystem::new());
    disk.insert("Road map.md", b"Ship the plan in spring. #project");
    disk.insert("ideas.md", b"We decided to keep the plan simple. #project");
    disk.insert("journal/today.md", b"Nothing new.");
    disk
}

#[gpui_kit::test]
fn typing_a_name_then_enter_opens_that_note(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::Files, cx);
    harness.type_text("road", cx);
    harness.press("enter", cx);
    assert_eq!(
        harness.events(),
        [WorkbenchEvent::Open(OpenRequest::new(path("Road map.md")))]
    );
    assert_eq!(*harness.dismissed.borrow(), 1);
    assert!(
        !harness
            .spotlight
            .read_with(cx, |spotlight, _| spotlight.is_open())
    );
}

#[gpui_kit::test]
fn a_content_hit_opens_in_a_new_tab_at_the_search_text(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::FullText, cx);
    harness.type_text("decided", cx);
    harness.press("secondary-enter", cx);
    assert_eq!(
        harness.events(),
        [WorkbenchEvent::Open(
            OpenRequest::new(path("ideas.md"))
                .in_target(OpenTarget::NewTab)
                .at_anchor("decided")
        )]
    );
}

#[gpui_kit::test]
fn a_tag_lists_its_notes_and_escape_returns_to_the_tags(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::Files, cx);
    harness.type_text("#proj", cx);
    harness.press("enter", cx);
    assert!(harness.events().is_empty());
    harness.press("down", cx);
    harness.press("escape", cx);
    assert_eq!(*harness.dismissed.borrow(), 0);
    harness.press("enter", cx);
    harness.press("down", cx);
    harness.press("enter", cx);
    assert_eq!(
        harness.events(),
        [WorkbenchEvent::Open(OpenRequest::new(path("ideas.md")))]
    );
}

#[gpui_kit::test]
fn replace_all_rewrites_every_matching_note(cx: &mut TestAppContext) {
    let disk = vault();
    let harness = Harness::open(&disk, SpotlightMode::Replace, cx);
    harness.type_text("plan", cx);
    cx.update_window(harness.window, |_, window, cx| {
        window.click("spotlight-replacement", cx);
        window.input("goal", cx);
        window.click("spotlight-replace-all", cx);
    })
    .expect("the window is open");
    harness.settle(cx);
    assert_eq!(
        disk.contents("Road map.md").as_deref(),
        Some(b"Ship the goal in spring. #project".as_slice())
    );
    assert_eq!(
        disk.contents("ideas.md").as_deref(),
        Some(b"We decided to keep the goal simple. #project".as_slice())
    );
    let events = harness.events();
    assert!(events.contains(&WorkbenchEvent::Saved(path("Road map.md"))));
    assert!(events.contains(&WorkbenchEvent::Saved(path("ideas.md"))));
    assert!(
        harness
            .spotlight
            .read_with(cx, |spotlight, _| spotlight.is_open())
    );
}

#[gpui_kit::test]
fn escape_dismisses(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::Files, cx);
    harness.press("escape", cx);
    assert_eq!(*harness.dismissed.borrow(), 1);
    assert!(harness.events().is_empty());
}

#[gpui_kit::test]
fn opening_again_while_open_keeps_the_query(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::Files, cx);
    let reopened = Rc::new(RefCell::new(0));
    let counter = Rc::clone(&reopened);
    cx.update(|cx| {
        cx.on_action(move |_: &OpenSpotlight, _| *counter.borrow_mut() += 1);
    });
    harness.type_text("road", cx);
    cx.update_window(harness.window, |_, window, cx| {
        window.dispatch_action(Box::new(OpenSpotlight), cx);
    })
    .expect("the window is open");
    harness.settle(cx);
    assert_eq!(*reopened.borrow(), 0);
    harness.press("enter", cx);
    assert_eq!(
        harness.events(),
        [WorkbenchEvent::Open(OpenRequest::new(path("Road map.md")))]
    );
}

#[gpui_kit::test]
fn replace_in_note_toggles_the_replace_row_outside_tag_mode(cx: &mut TestAppContext) {
    let harness = Harness::open(&vault(), SpotlightMode::Files, cx);
    let toggle = |cx: &mut TestAppContext| {
        cx.update_window(harness.window, |_, window, cx| {
            window.dispatch_action(Box::new(ReplaceInNote), cx);
        })
        .expect("the window is open");
        harness.settle(cx);
    };
    let shows = |cx: &mut TestAppContext| {
        cx.update_window(harness.window, |_, window, _| {
            window.try_find("spotlight-replacement").is_some()
        })
        .expect("the window is open")
    };
    harness.type_text("plan", cx);
    toggle(cx);
    assert!(shows(cx));
    toggle(cx);
    assert!(!shows(cx));
    for _ in 0.."plan".len() {
        harness.press("backspace", cx);
    }
    harness.type_text("#", cx);
    toggle(cx);
    assert!(!shows(cx));
}
