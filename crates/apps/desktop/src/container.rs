use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use margin_ask::AskService;
use margin_attachments::AttachmentsService;
use margin_crypto::DerivedVault;
use margin_files::FilesService;
use margin_fs::{NotifyWatcher, StdFileSystem, StdHostFiles, SystemClock};
use margin_history::History;
use margin_kernel::{Clock, FileSystem, HistoryStore, RelativePath, StoreError, Watcher};
use margin_llm::HttpLanguageModel;
use margin_pty::NativePtyHost;
use margin_s3::S3BlobStore;
use margin_search::SearchService;
use margin_settings::{S3Config, Settings};
use margin_sqlite::SqliteIndex;
use margin_sync::{SnapshotStore, SyncService};
use margin_terminals::Terminals;
use margin_trash::TrashService;
use margin_vaults::VaultService;
use margin_workbench::{AppServices, SyncFactory, VaultServices};
use parking_lot::RwLock;
use thiserror::Error;

use crate::history_snapshots::HistorySnapshots;
use crate::network::TokioTimer;
use crate::vault_reader::VaultBridge;

const INDEX_DATABASE: &str = ".margin/index.sqlite";
const DOCS_FOLDER: &str = ".margin/docs";

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Settings(#[from] margin_settings::Error),
}

#[must_use]
pub fn app_services(data_dir: PathBuf) -> AppServices {
    let app_data: Arc<dyn FileSystem> = Arc::new(StdFileSystem::new(data_dir.clone()));
    AppServices {
        vaults: Arc::new(VaultService::new(app_data)),
        host_files: Arc::new(StdHostFiles::new()),
        clock: Arc::new(SystemClock::new()),
        data_dir,
    }
}

pub struct OpenedVault {
    pub services: Arc<VaultServices>,
    pub watcher: Arc<dyn Watcher>,
}

impl fmt::Debug for OpenedVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedVault")
            .field("services", &self.services)
            .finish_non_exhaustive()
    }
}

pub fn open_vault(
    app: &AppServices,
    name: String,
    root: &Path,
    vault: &DerivedVault,
) -> Result<OpenedVault, ContainerError> {
    let root = root.to_path_buf();
    let files: Arc<dyn FileSystem> = Arc::new(StdFileSystem::new(root.clone()));
    files.create_dir_all(
        &RelativePath::new(DOCS_FOLDER)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    )?;
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let history = Arc::new(History::new(Arc::clone(&files), Arc::clone(&clock)));
    let history_store: Arc<dyn HistoryStore> = Arc::<History>::clone(&history);
    let trash = Arc::new(TrashService::new(
        Arc::clone(&files),
        Arc::clone(&history_store),
        Arc::clone(&clock),
    ));
    let listing = Arc::new(FilesService::new(
        Arc::clone(&files),
        Arc::clone(&app.host_files),
        history_store,
        Arc::clone(&trash),
        root.clone(),
    ));
    let attachments = Arc::new(AttachmentsService::new(
        Arc::clone(&files),
        Arc::clone(&app.host_files),
        Arc::clone(&trash),
        Arc::clone(&clock),
    ));
    let index = Arc::new(SqliteIndex::open(root.join(INDEX_DATABASE))?);
    let search = Arc::new(SearchService::new(index, Arc::clone(&files)));
    let settings = Arc::new(Settings::new(Arc::clone(&files)));
    let app_settings = settings.load(&vault.key)?.unwrap_or_default();
    let reader = Arc::new(VaultBridge::new(
        Arc::clone(&files),
        Arc::clone(&listing),
        Arc::clone(&search),
    ));
    let ask = Arc::new(AskService::new(
        Arc::new(HttpLanguageModel::new()),
        reader,
        Arc::clone(&clock),
    ));
    if let Some(llm) = app_settings.llm.clone() {
        drop(ask.configure(llm));
    }
    let factory = Arc::new(S3SyncFactory {
        files: Arc::clone(&files),
        clock: Arc::clone(&clock),
        snapshots: Arc::new(HistorySnapshots::new(Arc::clone(&history))),
        vault: vault.clone(),
    });
    let services = Arc::new(VaultServices {
        name,
        id: vault.id.clone(),
        key: vault.key,
        files: listing,
        trash,
        attachments,
        search,
        history,
        settings,
        app_settings: Arc::new(RwLock::new(app_settings)),
        sync: Arc::new(RwLock::new(None)),
        sync_factory: factory,
        ask,
        terminals: Terminals::new(Arc::new(NativePtyHost)),
        root: root.clone(),
    });
    services.configure_sync();
    Ok(OpenedVault {
        services,
        watcher: Arc::new(NotifyWatcher::new(root)),
    })
}

struct S3SyncFactory {
    files: Arc<dyn FileSystem>,
    clock: Arc<dyn Clock>,
    snapshots: Arc<dyn SnapshotStore>,
    vault: DerivedVault,
}

impl SyncFactory for S3SyncFactory {
    fn build(&self, config: &S3Config) -> Arc<SyncService> {
        let store = S3BlobStore::new(margin_s3::S3Config::new(
            config.endpoint.clone(),
            config.bucket.clone(),
            config.region.clone(),
            config.access_key.clone(),
            config.secret_key.clone(),
        ));
        Arc::new(SyncService::new(
            Arc::clone(&self.files),
            Arc::new(store),
            Arc::clone(&self.clock),
            Arc::new(TokioTimer),
            Arc::clone(&self.snapshots),
            self.vault.clone(),
        ))
    }
}
