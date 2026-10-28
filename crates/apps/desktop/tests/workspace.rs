use std::fs;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, TestAppContext, VisualTestContext, Window,
    WindowBounds, WindowOptions, point, px, size,
};
use margin_crypto::{VaultKey, derive_vault};
use margin_desktop::{MarginWindow, app_services, init_with_data_dir};
use margin_fs::StdFileSystem;
use margin_kernel::{RelativePath, Secret};
use margin_search::SearchMode;
use margin_settings::{Settings, WorkspacePane, WorkspaceState, WorkspaceTab};
use margin_workbench::{CurrentVault, Updates};
use tempfile::TempDir;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const OTHER_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";
const ALPHA: &str = "# Alpha\n\nFirst draft.\n";
const BETA: &str = "# Beta\n\nNothing links here yet.\n";
const SETTLE: Duration = Duration::from_millis(200);
const AUTOSAVE: Duration = Duration::from_secs(1);

struct Fixture {
    data: TempDir,
    vault: TempDir,
}

impl Fixture {
    fn with_profile(mnemonic: &str) -> Self {
        let data = TempDir::new().expect("the data directory is created");
        let vault = TempDir::new().expect("the vault directory is created");
        fs::write(vault.path().join("alpha.md"), ALPHA).expect("alpha is seeded");
        fs::write(vault.path().join("beta.md"), BETA).expect("beta is seeded");
        let folder = vault.path().to_str().expect("the temporary path is UTF-8");
        app_services(data.path().to_path_buf())
            .vaults
            .save_profile("Notes", folder, &Secret::new(mnemonic.to_owned()))
            .expect("the profile is saved");
        Self { data, vault }
    }

    fn note(&self, name: &str) -> String {
        fs::read_to_string(self.vault.path().join(name)).expect("the note exists")
    }

    fn settings(&self) -> Settings {
        Settings::new(Arc::new(StdFileSystem::new(
            self.vault.path().to_path_buf(),
        )))
    }

    fn saved_workspace(&self) -> WorkspaceState {
        self.settings()
            .load_workspace(&key())
            .expect("the workspace state decrypts")
            .expect("the workspace state was saved")
    }
}

fn key() -> VaultKey {
    derive_vault(MNEMONIC).expect("the mnemonic is valid").key
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("the path is valid")
}

fn accept_wakeups_from_the_real_file_watcher_thread(cx: &TestAppContext) {
    cx.executor().allow_parking();
}

fn launch(fixture: &Fixture, cx: &TestAppContext) -> AnyWindowHandle {
    accept_wakeups_from_the_real_file_watcher_thread(cx);
    let data = fixture.data.path().to_path_buf();
    let window = cx.update(|cx| {
        init_with_data_dir(data, cx);
        drop(cx.remove_global::<Updates>());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(1200.0), px(800.0)),
            })),
            ..WindowOptions::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| MarginWindow::new(window, cx))
        })
        .expect("the headless window opens")
        .0
    });
    settle(cx);
    window
}

fn settle(cx: &TestAppContext) {
    cx.executor().advance_clock(SETTLE);
    cx.run_until_parked();
}

fn wait_for_autosave(cx: &TestAppContext) {
    cx.executor().advance_clock(AUTOSAVE);
    cx.run_until_parked();
}

fn act(window: AnyWindowHandle, cx: &mut TestAppContext, step: impl FnOnce(&mut Window, &mut App)) {
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        step(window, cx);
    })
    .expect("the window is open");
    settle(cx);
}

fn press(window: AnyWindowHandle, cx: &mut TestAppContext, key: &str) {
    act(window, cx, |window, cx| window.press(key, cx));
}

fn type_text(window: AnyWindowHandle, cx: &mut TestAppContext, text: &str) {
    act(window, cx, |window, cx| window.input(text, cx));
}

fn open_with_spotlight(window: AnyWindowHandle, cx: &mut TestAppContext, query: &str, key: &str) {
    press(window, cx, "secondary-p");
    type_text(window, cx, query);
    press(window, cx, key);
}

fn close(window: AnyWindowHandle, cx: &mut TestAppContext) {
    let closed_at_once = VisualTestContext::from_window(window, cx).simulate_close();
    assert!(!closed_at_once, "the window first flushes the workspace");
    wait_for_autosave(cx);
    assert!(
        cx.update_window(window, |_, _, _| ()).is_err(),
        "the window closes once everything is saved"
    );
}

fn tab(path: &str, view_mode: &str) -> WorkspaceTab {
    WorkspaceTab {
        path: path.to_owned(),
        tab_type: "markdown".to_owned(),
        pinned: false,
        view_mode: view_mode.to_owned(),
        cursor_pos: None,
    }
}

fn without_cursors(state: &WorkspaceState) -> WorkspaceState {
    let mut state = state.clone();
    for tab in state.panes.iter_mut().flat_map(|pane| &mut pane.tabs) {
        tab.cursor_pos = None;
    }
    state
}

#[gpui_kit::test]
fn typing_in_a_note_saves_it_with_history_and_updates_search_and_backlinks(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::with_profile(MNEMONIC);
    let window = launch(&fixture, cx);
    let services = cx.read(CurrentVault::expect);
    let (alpha, beta) = (path("alpha.md"), path("beta.md"));
    let search = |query: &str| {
        services
            .search
            .search(query, SearchMode::All, 10)
            .expect("the index answers")
            .into_iter()
            .map(|hit| hit.path)
            .collect::<Vec<_>>()
    };
    let backlinks = || {
        services
            .search
            .backlinks(&beta)
            .expect("the index answers")
            .into_iter()
            .map(|link| link.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(search("draft"), [path("alpha.md")]);
    assert!(search("comet").is_empty());
    assert!(backlinks().is_empty());

    open_with_spotlight(window, cx, "alpha", "enter");
    type_text(window, cx, "Comet [[beta]] ");
    wait_for_autosave(cx);

    assert_eq!(fixture.note("alpha.md"), format!("Comet [[beta]] {ALPHA}"));
    let snapshots = services.history.list(&alpha).expect("the history reads");
    let [snapshot] = snapshots.as_slice() else {
        panic!("the first save keeps the departing text: {snapshots:?}");
    };
    assert_eq!(
        services
            .history
            .read(&alpha, &snapshot.filename)
            .expect("the snapshot reads"),
        ALPHA.as_bytes()
    );
    assert_eq!(search("comet"), [path("alpha.md")]);
    assert_eq!(backlinks(), [alpha]);
    assert_eq!(fixture.note("beta.md"), BETA);
}

#[gpui_kit::test]
fn tabs_view_modes_and_the_sidebar_come_back_after_a_relaunch(cx: &mut TestAppContext) {
    let fixture = Fixture::with_profile(MNEMONIC);
    let window = launch(&fixture, cx);
    open_with_spotlight(window, cx, "alpha", "enter");
    open_with_spotlight(window, cx, "beta", "secondary-enter");
    press(window, cx, "secondary-shift-e");
    press(window, cx, "secondary-\\");
    close(window, cx);

    let saved = fixture.saved_workspace();
    assert_eq!(
        without_cursors(&saved).panes,
        [WorkspacePane {
            tabs: vec![tab("alpha.md", "rich"), tab("beta.md", "source")],
            active_tab_index: 1,
        }]
    );
    assert!(!saved.sidebar_open);

    let mut relaunched = cx.new_app();
    let window = launch(&fixture, &relaunched);
    type_text(window, &mut relaunched, "Again ");
    wait_for_autosave(&relaunched);
    assert!(
        fixture.note("beta.md").contains("Again "),
        "typing goes to the restored active tab: {}",
        fixture.note("beta.md")
    );
    assert_eq!(fixture.note("alpha.md"), ALPHA);
    close(window, &mut relaunched);
    assert_eq!(
        without_cursors(&fixture.saved_workspace()),
        without_cursors(&saved)
    );
}

#[gpui_kit::test]
fn a_split_layout_from_the_last_session_is_restored(cx: &mut TestAppContext) {
    let fixture = Fixture::with_profile(MNEMONIC);
    let saved = WorkspaceState {
        panes: vec![
            WorkspacePane {
                tabs: vec![tab("alpha.md", "rich")],
                active_tab_index: 0,
            },
            WorkspacePane {
                tabs: vec![tab("beta.md", "rich")],
                active_tab_index: 0,
            },
        ],
        pane_flexes: vec![1.0, 1.0],
        active_pane_index: 1,
        expanded_folders: Vec::new(),
        sidebar_open: true,
        sidebar_width: 300.0,
        sort_order: "name".to_owned(),
        terminal_open: false,
        terminal_height: 280.0,
    };
    fixture
        .settings()
        .save_workspace(&key(), &saved)
        .expect("the last session's layout is saved");

    let window = launch(&fixture, cx);
    type_text(window, cx, "Split ");
    wait_for_autosave(cx);
    assert_eq!(fixture.note("beta.md"), format!("Split {BETA}"));
    assert_eq!(fixture.note("alpha.md"), ALPHA);
    close(window, cx);

    assert_eq!(without_cursors(&fixture.saved_workspace()), saved);
}

#[gpui_kit::test]
fn a_folder_claimed_by_another_passphrase_stays_locked(cx: &mut TestAppContext) {
    let fixture = Fixture::with_profile(OTHER_MNEMONIC);
    let marker = fixture.vault.path().join(".margin").join("vault.id");
    let owner = derive_vault(MNEMONIC).expect("the mnemonic is valid").id;
    fs::create_dir_all(marker.parent().expect("the marker has a folder"))
        .expect("the vault folder is marked");
    fs::write(&marker, owner.as_str()).expect("the vault folder is marked");

    let window = launch(&fixture, cx);

    assert!(cx.read(CurrentVault::get).is_none());
    assert_eq!(
        fs::read_to_string(&marker).expect("the marker stays"),
        owner.as_str()
    );
    assert!(!fixture.vault.path().join(".margin/index.sqlite").exists());
    act(window, cx, |window, _| {
        assert!(window.try_find(("vault-profile", 0_usize)).is_some());
    });
}
