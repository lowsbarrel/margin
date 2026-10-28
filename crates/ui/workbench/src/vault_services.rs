use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{App, Global};
use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::{VaultId, VaultKey};
use margin_files::FilesService;
use margin_history::History;
use margin_kernel::{Clock, HostFiles};
use margin_search::SearchService;
use margin_settings::{AppSettings, Settings};
use margin_sync::SyncService;
use margin_terminals::Terminals;
use margin_trash::TrashService;
use margin_vaults::VaultService;
use parking_lot::RwLock;

use crate::sync_factory::SyncFactory;

#[derive(Clone)]
pub struct AppServices {
    pub vaults: Arc<VaultService>,
    pub host_files: Arc<dyn HostFiles>,
    pub clock: Arc<dyn Clock>,
    pub data_dir: PathBuf,
}

impl Global for AppServices {}

impl fmt::Debug for AppServices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppServices")
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

pub struct VaultServices {
    pub root: PathBuf,
    pub name: String,
    pub id: VaultId,
    pub key: VaultKey,
    pub files: Arc<FilesService>,
    pub trash: Arc<TrashService>,
    pub attachments: Arc<AttachmentsService>,
    pub search: Arc<SearchService>,
    pub history: Arc<History>,
    pub settings: Arc<Settings>,
    pub app_settings: Arc<RwLock<AppSettings>>,
    pub sync: Arc<RwLock<Option<Arc<SyncService>>>>,
    pub sync_factory: Arc<dyn SyncFactory>,
    pub ask: Arc<AskService>,
    pub terminals: Terminals,
}

impl fmt::Debug for VaultServices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultServices")
            .field("root", &self.root)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl VaultServices {
    pub fn configure_sync(&self) {
        let config = self.app_settings.read().s3.clone();
        let service = config
            .as_ref()
            .map(|config| self.sync_factory.build(config));
        *self.sync.write() = service;
    }

    #[must_use]
    pub fn sync_service(&self) -> Option<Arc<SyncService>> {
        self.sync.read().clone()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CurrentVault(pub Option<Arc<VaultServices>>);

impl Global for CurrentVault {}

impl CurrentVault {
    #[must_use]
    pub fn get(cx: &App) -> Option<Arc<VaultServices>> {
        cx.try_global::<Self>()
            .and_then(|current| current.0.clone())
    }

    #[must_use]
    pub fn expect(cx: &App) -> Arc<VaultServices> {
        Self::get(cx).expect("vault surfaces exist only while a vault is open")
    }
}
