use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::base::test_support;
use gpui_kit::component::Root;
use gpui_kit::{
    AppContext, Context, ElementId, IntoElement, Modifiers, Render, TestAppContext,
    VisualTestContext, Window, div,
};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::{DerivedVault, derive_vault};
use margin_files::FilesService;
use margin_history::History;
use margin_i18n::{Locale, Translator};
use margin_kernel::{Clock, FileSystem, HistoryStore, HostFiles};
use margin_preferences::{SettingsSection, open_settings_section};
use margin_search::SearchService;
use margin_settings::{AppSettings, S3Config, Settings};
use margin_sync::SyncService;
use margin_terminals::Terminals;
use margin_test_support::{
    FakePtyHost, FixedClock, InstantTimer, MemoryBlobStore, MemoryFileSystem, MemoryHostFiles,
    MemorySearchIndex, MemorySnapshotStore, MemoryVaultReader, ScriptedLanguageModel,
};
use margin_theme::{Appearance, MarginTheme};
use margin_trash::TrashService;
use margin_workbench::{CurrentVault, SyncFactory, VaultServices};
use parking_lot::RwLock;

const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                        abandon abandon about";

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div()
    }
}

struct MemorySync {
    files: Arc<dyn FileSystem>,
    vault: DerivedVault,
}

impl SyncFactory for MemorySync {
    fn build(&self, _: &S3Config) -> Arc<SyncService> {
        Arc::new(SyncService::new(
            Arc::clone(&self.files),
            Arc::new(MemoryBlobStore::new()),
            Arc::new(FixedClock::default()),
            Arc::new(InstantTimer::new()),
            Arc::new(MemorySnapshotStore::new()),
            self.vault.clone(),
        ))
    }
}

fn vault_services() -> VaultServices {
    let vault = derive_vault(MNEMONIC).expect("the test mnemonic is valid");
    let root = PathBuf::from("/vault");
    let files: Arc<dyn FileSystem> = Arc::new(MemoryFileSystem::new());
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::default());
    let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
    let history = Arc::new(History::new(Arc::clone(&files), Arc::clone(&clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(&files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
    ));
    VaultServices {
        root: root.clone(),
        name: "Notes".to_owned(),
        id: vault.id.clone(),
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
        app_settings: Arc::new(RwLock::new(AppSettings {
            s3: None,
            attachment_folder: None,
            auto_sync: None,
            conflict_strategy: None,
            llm: None,
            locale: None,
            theme: Some(Appearance::Light.as_setting().to_owned()),
        })),
        sync: Arc::new(RwLock::new(None)),
        sync_factory: Arc::new(MemorySync {
            files: Arc::clone(&files),
            vault,
        }),
        ask: Arc::new(AskService::new(
            Arc::new(ScriptedLanguageModel::new(Vec::new())),
            Arc::new(MemoryVaultReader::new()),
            clock,
        )),
        terminals: Terminals::new(Arc::new(FakePtyHost::new())),
    }
}

fn open(
    cx: &mut TestAppContext,
    section: SettingsSection,
) -> (Arc<VaultServices>, &mut VisualTestContext) {
    let services = Arc::new(vault_services());
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Translator::init(Some(Locale::English), cx);
        MarginTheme::init(Appearance::Light, cx);
        cx.set_global(CurrentVault(Some(Arc::clone(&services))));
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        Root::new(host, window, cx)
    });
    cx.update(|window, cx| open_settings_section(section, window, cx));
    repaint(cx);
    (services, cx)
}

fn repaint(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn painted(cx: &mut VisualTestContext, id: &'static str) -> bool {
    cx.update(|window, _| test_support::find(window, &[], &ElementId::from(id)).is_some())
}

fn click(cx: &mut VisualTestContext, id: &'static str) {
    let target = cx.update(|window, _| {
        test_support::find(window, &[], &ElementId::from(id))
            .unwrap_or_else(|| panic!("{id} is painted"))
    });
    cx.simulate_click(target.bounds().center(), Modifiers::none());
    repaint(cx);
}

fn stored(services: &VaultServices) -> Option<AppSettings> {
    services
        .settings
        .load(&services.key)
        .expect("the settings decrypt")
}

#[gpui_kit::test]
fn choosing_dark_appearance_applies_and_persists_it(cx: &mut TestAppContext) {
    let (services, cx) = open(cx, SettingsSection::Appearance);
    click(cx, "appearance_dark");

    cx.update(|_, cx| {
        assert_eq!(MarginTheme::global(cx).appearance(), Appearance::Dark);
        assert!(MarginTheme::palette(cx).dark);
    });
    let stored = stored(&services).expect("the settings were written");
    assert_eq!(stored.theme.as_deref(), Some("dark"));
}

#[gpui_kit::test]
fn only_the_requested_section_starts_open_and_headers_toggle(cx: &mut TestAppContext) {
    let (_, cx) = open(cx, SettingsSection::Vault);
    assert!(painted(cx, "rename-vault"));
    assert!(!painted(cx, "test-connection"));

    click(cx, "settings_s3_title");
    assert!(painted(cx, "test-connection"));
    assert!(painted(cx, "rename-vault"));

    click(cx, "settings_vault_title");
    assert!(!painted(cx, "rename-vault"));
}

#[gpui_kit::test]
fn closing_saves_only_when_something_changed(cx: &mut TestAppContext) {
    let (services, cx) = open(cx, SettingsSection::Sync);
    click(cx, "close-settings");
    assert!(
        stored(&services).is_none(),
        "an untouched form writes nothing"
    );

    cx.update(|window, cx| open_settings_section(SettingsSection::Sync, window, cx));
    repaint(cx);
    click(cx, "auto-sync-label");
    cx.simulate_keystrokes("escape");
    repaint(cx);
    let stored = stored(&services).expect("closing wrote the change");
    assert_eq!(stored.auto_sync, Some(true));
    assert_eq!(services.app_settings.read().auto_sync, Some(true));
}
