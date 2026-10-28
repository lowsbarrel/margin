use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_canvas::CanvasView;
use margin_crypto::derive_vault;
use margin_files::FilesService;
use margin_history::History;
use margin_i18n::{Locale, Translator};
use margin_kernel::{Clock, FileSystem, HistoryStore, HostFiles, RelativePath};
use margin_search::SearchService;
use margin_settings::{AppSettings, S3Config, Settings};
use margin_sync::SyncService;
use margin_terminals::Terminals;
use margin_test_support::{
    FakePtyHost, FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles,
    MemorySearchIndex, MemoryVaultReader, ScriptedLanguageModel,
};
use margin_theme::{Appearance, MarginTheme};
use margin_trash::TrashService;
use margin_workbench::{CurrentVault, SyncFactory, TabContent, VaultServices};
use parking_lot::RwLock;

#[derive(Debug)]
struct NoSync;

impl SyncFactory for NoSync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        panic!("canvas tests never configure sync")
    }
}

fn vault(memory: &Arc<MemoryFileSystem>) -> Arc<VaultServices> {
    let files: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(memory);
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let history_store: Arc<dyn HistoryStore> =
        Arc::new(MemoryHistoryStore::new(Arc::clone(memory)));
    let trash = Arc::new(TrashService::new(
        Arc::clone(&files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
    ));
    let vault = derive_vault(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("the test passphrase is valid");
    Arc::new(VaultServices {
        root: PathBuf::from("vault"),
        name: "Test".to_owned(),
        id: vault.id,
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
        search: Arc::new(SearchService::new(
            Arc::new(MemorySearchIndex::new()),
            Arc::clone(&files),
        )),
        history: Arc::new(History::new(Arc::clone(&files), Arc::clone(&clock))),
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
        sync_factory: Arc::new(NoSync),
        ask: Arc::new(AskService::new(
            Arc::new(ScriptedLanguageModel::new(Vec::new())),
            Arc::new(MemoryVaultReader::new()),
            clock,
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    })
}

const BOARD: &str = "board.canvas";

fn setup(cx: &TestAppContext) -> Arc<MemoryFileSystem> {
    let memory = Arc::new(MemoryFileSystem::new());
    let services = vault(&memory);
    cx.update(|cx| {
        gpui_kit::init(cx);
        MarginTheme::init(Appearance::Light, cx);
        Translator::init(Some(Locale::English), cx);
        cx.set_global(CurrentVault(Some(services)));
    });
    memory
}

fn open(cx: &mut TestAppContext) -> (Entity<CanvasView>, &mut VisualTestContext) {
    let path = RelativePath::new(BOARD).expect("valid path");
    let (view, cx) = cx.add_window_view(|window, cx| CanvasView::new(path, window, cx));
    cx.run_until_parked();
    (view, cx)
}

fn drag(cx: &mut VisualTestContext, points: &[(f32, f32)]) {
    let at = |(x, y): (f32, f32)| point(px(x), px(y));
    cx.simulate_mouse_down(at(points[0]), MouseButton::Left, Modifiers::none());
    for position in &points[1..] {
        cx.simulate_mouse_move(at(*position), MouseButton::Left, Modifiers::none());
    }
    let last = points[points.len() - 1];
    cx.simulate_mouse_up(at(last), MouseButton::Left, Modifiers::none());
}

fn right_click(cx: &mut VisualTestContext, x: f32, y: f32) {
    let at = point(px(x), px(y));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

fn click_selector(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is rendered"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

fn saved(
    cx: &mut VisualTestContext,
    view: &Entity<CanvasView>,
    memory: &MemoryFileSystem,
) -> serde_json::Value {
    view.update(cx, TabContent::flush);
    cx.run_until_parked();
    let path = RelativePath::new(BOARD).expect("valid path");
    serde_json::from_slice(&memory.read(&path).expect("flush wrote the canvas"))
        .expect("the canvas is JSON")
}

#[gpui_kit::test]
fn pen_drag_adds_a_stroke_and_flush_writes_the_file(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    drag(cx, &[(100.0, 100.0), (140.0, 120.0), (180.0, 160.0)]);
    assert!(view.read_with(cx, |view, _| view.is_dirty()));

    let saved = saved(cx, &view, &memory);
    let stroke = &saved["strokes"][0];
    let points: Vec<(f64, f64)> = stroke["points"]
        .as_array()
        .expect("strokes have points")
        .iter()
        .map(|point| {
            (
                point["x"].as_f64().expect("x"),
                point["y"].as_f64().expect("y"),
            )
        })
        .collect();
    let top = points[0].1;
    assert_eq!(
        points,
        vec![(100.0, top), (140.0, top + 20.0), (180.0, top + 60.0)]
    );
    assert_eq!(stroke["tool"], "pen");
    assert_eq!(stroke["color"], "ink");
    assert_eq!(stroke["size"], 3);
    assert_eq!(saved["zoom"], 1);
    assert!(!view.read_with(cx, |view, _| view.is_dirty()));
}

#[gpui_kit::test]
fn eraser_saves_an_eraser_stroke_over_the_ink(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    drag(cx, &[(100.0, 200.0), (300.0, 200.0)]);
    cx.simulate_keystrokes("e");
    drag(cx, &[(200.0, 150.0), (200.0, 250.0)]);

    let saved = saved(cx, &view, &memory);
    let strokes = saved["strokes"].as_array().expect("strokes");
    assert_eq!(strokes.len(), 2);
    assert_eq!(strokes[0]["tool"], "pen");
    assert_eq!(strokes[1]["tool"], "eraser");
    assert_eq!(strokes[1]["color"], "#000");
    assert_eq!(strokes[1]["size"], 20);
}

#[gpui_kit::test]
fn text_tool_click_type_enter_adds_a_label(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    cx.simulate_keystrokes("t");
    cx.simulate_click(point(px(150.0), px(150.0)), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_input("Hello");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let saved = saved(cx, &view, &memory);
    let label = &saved["textLabels"][0];
    assert_eq!(label["text"], "Hello");
    assert_eq!(label["x"], 150);
    assert_eq!(label["fontSize"], 16);
    assert_eq!(label["color"], "ink");

    cx.simulate_keystrokes("cmd-z");
    cx.run_until_parked();
    let saved = self::saved(cx, &view, &memory);
    assert!(saved["textLabels"].as_array().expect("labels").is_empty());
}

#[gpui_kit::test]
fn hand_drag_pans_and_cmd_z_undoes_the_last_edit(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    drag(cx, &[(100.0, 100.0), (200.0, 100.0)]);
    drag(cx, &[(100.0, 300.0), (200.0, 300.0)]);
    cx.simulate_keystrokes("cmd-z h");
    drag(cx, &[(150.0, 100.0), (110.0, 70.0)]);

    let saved = saved(cx, &view, &memory);
    assert_eq!(saved["strokes"].as_array().expect("strokes").len(), 1);
    assert_eq!(saved["camX"], 40);
    assert_eq!(saved["camY"], 30);
}

#[gpui_kit::test]
fn context_menu_clears_only_after_inline_confirmation(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    drag(cx, &[(100.0, 100.0), (200.0, 100.0)]);

    right_click(cx, 300.0, 300.0);
    click_selector(cx, "canvas-clear-all");
    click_selector(cx, "canvas-clear-no");
    assert!(cx.debug_bounds("canvas-clear-no").is_none());
    assert_eq!(saved(cx, &view, &memory)["strokes"][0]["tool"], "pen");

    right_click(cx, 300.0, 300.0);
    cx.simulate_keystrokes("escape");
    assert!(cx.debug_bounds("canvas-clear-all").is_none());

    right_click(cx, 300.0, 300.0);
    click_selector(cx, "canvas-clear-all");
    click_selector(cx, "canvas-clear-yes");
    assert!(cx.debug_bounds("canvas-clear-all").is_none());
    let saved = saved(cx, &view, &memory);
    assert!(saved["strokes"].as_array().expect("strokes").is_empty());
}

#[gpui_kit::test]
fn reloading_its_own_save_keeps_the_undo_history(cx: &mut TestAppContext) {
    let memory = setup(cx);
    let (view, cx) = open(cx);
    drag(cx, &[(100.0, 100.0), (200.0, 100.0)]);
    drag(cx, &[(100.0, 300.0), (200.0, 300.0)]);
    drop(saved(cx, &view, &memory));
    view.update(cx, TabContent::reload);
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-z");
    let saved = saved(cx, &view, &memory);
    assert_eq!(saved["strokes"].as_array().expect("strokes").len(), 1);
}
