mod app;
mod container;
mod history_snapshots;
mod lifecycle;
#[cfg(target_os = "macos")]
mod menus;
mod network;
mod pane_events;
mod shell_layout;
mod status_bar;
mod tab_view;
mod update_install;
mod update_notice;
mod updater;
mod vault_reader;
mod window;
mod workspace;
mod workspace_actions;
mod workspace_state;

pub use app::{init, init_with_data_dir, run};
pub use container::{ContainerError, OpenedVault, app_services, open_vault};
pub use window::MarginWindow;
