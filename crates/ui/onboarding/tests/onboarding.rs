use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::{env, fs};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ExternalPaths, FileDropEvent, InputEvent, TestAppContext,
    Window, WindowBounds, WindowOptions, point, px, size,
};
use margin_crypto::derive_vault;
use margin_i18n::{Locale, Translator};
use margin_kernel::Secret;
use margin_onboarding::{Onboarding, OnboardingEvent};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHostFiles};
use margin_theme::{Appearance, MarginTheme};
use margin_vaults::VaultService;
use margin_workbench::AppServices;

const WORDS: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const FOLDER: &str = "/home/me/Notes";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Start {
    WithAutoLogin,
    WithoutAutoLogin,
}

struct Harness {
    window: AnyWindowHandle,
    events: Rc<RefCell<Vec<OnboardingEvent>>>,
    vaults: Arc<VaultService>,
    host: Arc<MemoryHostFiles>,
}

impl Harness {
    fn services(cx: &TestAppContext) -> (Arc<VaultService>, Arc<MemoryHostFiles>) {
        let vaults = Arc::new(VaultService::new(Arc::new(MemoryFileSystem::new())));
        let host = Arc::new(MemoryHostFiles::new());
        cx.update(|cx| {
            gpui_kit::init(cx);
            Translator::init(Some(Locale::English), cx);
            MarginTheme::init(Appearance::Light, cx);
            cx.set_global(AppServices {
                vaults: Arc::clone(&vaults),
                host_files: Arc::<MemoryHostFiles>::clone(&host),
                clock: Arc::new(FixedClock::default()),
                data_dir: PathBuf::from("/data"),
            });
        });
        (vaults, host)
    }

    fn open(
        start: Start,
        vaults: Arc<VaultService>,
        host: Arc<MemoryHostFiles>,
        cx: &TestAppContext,
    ) -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&events);
        let window = cx.update(|cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(900.0), px(900.0)),
                })),
                ..WindowOptions::default()
            };
            let (window, view) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| match start {
                    Start::WithAutoLogin => Onboarding::new(window, cx),
                    Start::WithoutAutoLogin => Onboarding::new_without_auto_login(window, cx),
                })
            })
            .expect("the test window opens");
            cx.subscribe(&view, move |_, event: &OnboardingEvent, _| {
                recorded.borrow_mut().push(event.clone());
            })
            .detach();
            window
        });
        cx.run_until_parked();
        Self {
            window,
            events,
            vaults,
            host,
        }
    }

    fn act(&self, cx: &mut TestAppContext, action: impl FnOnce(&mut Window, &mut gpui_kit::App)) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            action(window, cx);
        })
        .expect("the test window is open");
        cx.run_until_parked();
    }

    fn choose_folder(cx: &TestAppContext) {
        cx.simulate_path_prompt_response(|_| Some(vec![PathBuf::from(FOLDER)]));
        cx.run_until_parked();
    }

    fn unlocked(&self) -> Vec<OnboardingEvent> {
        self.events.borrow().clone()
    }
}

#[gpui_kit::test]
fn completing_the_wizard_saves_the_profile_and_unlocks_the_vault(cx: &mut TestAppContext) {
    let (vaults, host) = Harness::services(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| {
        window.click("onboarding-skip", cx);
        window.click("onboarding-pick-folder", cx);
    });
    Harness::choose_folder(cx);
    harness.act(cx, |window, cx| {
        window.click("onboarding-wizard-next", cx);
        window.render_frame(cx);
        assert_eq!(window.find("onboarding-vault-name").value(), Some("Notes"));
        window.click("onboarding-wizard-next", cx);
        window.input(WORDS, cx);
        window.click("onboarding-open-vault", cx);
    });

    let expected = derive_vault(WORDS).expect("the fixture is a valid passphrase");
    assert_eq!(
        harness.unlocked(),
        vec![OnboardingEvent::Unlocked {
            name: String::from("Notes"),
            root: PathBuf::from(FOLDER),
            vault: expected.clone(),
        }]
    );
    let profiles = harness.vaults.load_profiles().expect("the registry reads");
    assert_eq!(profiles.profiles.len(), 1);
    assert_eq!(profiles.profiles[0].name, "Notes");
    assert_eq!(profiles.last_used.as_deref(), Some(FOLDER));
    assert_eq!(
        harness
            .host
            .contents(&Path::new(FOLDER).join(".margin").join("vault.id")),
        Some(expected.id.as_str().as_bytes().to_vec())
    );
}

#[gpui_kit::test]
fn the_keyboard_alone_creates_a_vault_from_a_generated_passphrase(cx: &mut TestAppContext) {
    let (vaults, host) = Harness::services(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| {
        window.press("escape", cx);
        window.press("enter", cx);
    });
    Harness::choose_folder(cx);
    harness.act(cx, |window, cx| {
        window.press("enter", cx);
        window.press("enter", cx);
        window.click("onboarding-generate-passphrase", cx);
        window.click("onboarding-use-passphrase", cx);
        window.press("enter", cx);
    });

    let mnemonic = harness
        .vaults
        .reveal_mnemonic(FOLDER)
        .expect("the new vault is unlocked");
    assert_eq!(mnemonic.expose().split_whitespace().count(), 12);
    let expected = derive_vault(mnemonic.expose()).expect("the generated words are valid");
    assert_eq!(
        harness.unlocked(),
        vec![OnboardingEvent::Unlocked {
            name: String::from("Notes"),
            root: PathBuf::from(FOLDER),
            vault: expected,
        }]
    );
}

#[gpui_kit::test]
fn an_invalid_passphrase_keeps_the_wizard_open(cx: &mut TestAppContext) {
    let (vaults, host) = Harness::services(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| {
        window.click("onboarding-skip", cx);
        window.click("onboarding-pick-folder", cx);
    });
    Harness::choose_folder(cx);
    harness.act(cx, |window, cx| {
        window.click("onboarding-wizard-next", cx);
        window.click("onboarding-wizard-next", cx);
        window.input("these are not the right words", cx);
        window.click("onboarding-open-vault", cx);
    });

    assert!(harness.unlocked().is_empty());
    assert!(
        harness
            .vaults
            .load_profiles()
            .expect("the registry reads")
            .profiles
            .is_empty()
    );
    harness.act(cx, |window, _| {
        assert!(window.try_find("onboarding-open-vault").is_some());
    });
}

fn saved_vault(cx: &TestAppContext) -> (Arc<VaultService>, Arc<MemoryHostFiles>) {
    let (vaults, host) = Harness::services(cx);
    vaults
        .save_profile("Notes", FOLDER, &Secret::new(String::from(WORDS)))
        .expect("the profile saves");
    (vaults, host)
}

#[gpui_kit::test]
fn startup_reopens_the_last_session(cx: &TestAppContext) {
    let (vaults, host) = saved_vault(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);

    assert_eq!(
        harness.unlocked(),
        vec![OnboardingEvent::Unlocked {
            name: String::from("Notes"),
            root: PathBuf::from(FOLDER),
            vault: derive_vault(WORDS).expect("the fixture is valid"),
        }]
    );
}

#[gpui_kit::test]
fn locking_shows_the_vault_list_instead_of_reopening(cx: &mut TestAppContext) {
    let (vaults, host) = saved_vault(cx);
    let harness = Harness::open(Start::WithoutAutoLogin, vaults, host, cx);

    assert!(harness.unlocked().is_empty());
    harness.act(cx, |window, cx| {
        window.click(("vault-profile", 0_usize), cx);
    });
    assert_eq!(harness.unlocked().len(), 1);
}

#[gpui_kit::test]
fn the_vault_list_opens_only_a_vault_chosen_with_the_arrow_keys(cx: &mut TestAppContext) {
    let (vaults, host) = saved_vault(cx);
    let harness = Harness::open(Start::WithoutAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| window.press("enter", cx));
    assert!(harness.unlocked().is_empty());
    harness.act(cx, |window, cx| {
        window.press("down", cx);
        window.press("enter", cx);
    });
    assert_eq!(harness.unlocked().len(), 1);
}

#[gpui_kit::test]
fn removing_a_vault_asks_first_then_shows_the_intro(cx: &mut TestAppContext) {
    let (vaults, host) = saved_vault(cx);
    let harness = Harness::open(Start::WithoutAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| {
        window.click(("vault-profile-remove", 0_usize), cx);
    });
    assert!(harness.unlocked().is_empty());
    assert_eq!(
        harness
            .vaults
            .load_profiles()
            .expect("reads")
            .profiles
            .len(),
        1
    );
    harness.act(cx, |window, cx| window.press("enter", cx));
    assert!(
        harness
            .vaults
            .load_profiles()
            .expect("reads")
            .profiles
            .is_empty()
    );
    harness.act(cx, |window, _| {
        assert!(window.try_find("onboarding-next").is_some());
    });
}

#[gpui_kit::test]
fn escape_returns_to_the_vault_list_unless_a_field_has_focus(cx: &mut TestAppContext) {
    let (vaults, host) = saved_vault(cx);
    let harness = Harness::open(Start::WithoutAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| window.click("onboarding-add-vault", cx));
    harness.act(cx, |window, cx| {
        assert!(window.try_find("onboarding-back-to-vaults").is_some());
        window.press("escape", cx);
    });
    harness.act(cx, |window, cx| {
        assert!(window.try_find(("vault-profile", 0_usize)).is_some());
        window.click("onboarding-add-vault", cx);
    });
    harness.act(cx, |window, cx| window.click("onboarding-pick-folder", cx));
    Harness::choose_folder(cx);
    harness.act(cx, |window, cx| window.click("onboarding-wizard-next", cx));
    harness.act(cx, |window, cx| window.press("escape", cx));
    harness.act(cx, |window, _| {
        assert!(window.try_find("onboarding-vault-name").is_some());
    });
}

#[gpui_kit::test]
fn the_intro_follows_the_arrow_keys(cx: &mut TestAppContext) {
    let (vaults, host) = Harness::services(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);

    harness.act(cx, |window, cx| {
        window.press("left", cx);
        assert!(window.try_find("onboarding-back").is_none());
        window.press("right", cx);
        assert!(window.try_find("onboarding-back").is_some());
        window.press("left", cx);
        assert!(window.try_find("onboarding-back").is_none());
        for _ in 0..5 {
            window.press("right", cx);
        }
    });
    harness.act(cx, |window, _| {
        assert!(window.try_find("onboarding-pick-folder").is_some());
    });
}

#[gpui_kit::test]
fn dropping_a_folder_from_the_os_chooses_it(cx: &mut TestAppContext) {
    let (vaults, host) = Harness::services(cx);
    let harness = Harness::open(Start::WithAutoLogin, vaults, host, cx);
    let folder = env::temp_dir().join("margin-onboarding-drop");
    fs::create_dir_all(&folder).expect("the drop folder exists");

    harness.act(cx, |window, cx| window.click("onboarding-skip", cx));
    harness.act(cx, |window, cx| {
        let position = window.find("onboarding-pick-folder").bounds().center();
        let paths = ExternalPaths(vec![folder.clone()].into());
        window.dispatch_event(
            FileDropEvent::Entered { position, paths }.to_platform_input(),
            cx,
        );
        window.dispatch_event(FileDropEvent::Submit { position }.to_platform_input(), cx);
    });
    harness.act(cx, |window, cx| {
        window.click("onboarding-wizard-next", cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("onboarding-vault-name").value(),
            Some("margin-onboarding-drop")
        );
    });
}
