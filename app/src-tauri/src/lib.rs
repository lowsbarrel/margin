mod crypto;
mod fs;
mod history;
mod s3;
mod session;
mod settings;
mod sync;
mod text;
mod text_transform;
mod themes;

use fs::{VaultPathState, VaultWatcherState, WatcherState};
use s3::S3State;
use std::sync::Mutex;
use tauri::Manager;

fn mime_from_ext(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("pdf") => "application/pdf",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(S3State(Mutex::new(None)))
        .manage(WatcherState(Mutex::new(None)))
        .manage(VaultWatcherState(Mutex::new(None)))
        .manage(VaultPathState(Mutex::new(String::new())))
        .register_uri_scheme_protocol("localfile", |_app, request| {
            let decoded = percent_encoding::percent_decode_str(request.uri().path())
                .decode_utf8_lossy()
                .into_owned();

            // On Windows the URL path looks like "/C:/Users/..." — strip the
            // leading slash so Path::canonicalize can resolve it.
            #[cfg(windows)]
            let decoded = {
                let bytes = decoded.as_bytes();
                if bytes.len() >= 3
                    && bytes[0] == b'/'
                    && bytes[1].is_ascii_alphabetic()
                    && bytes[2] == b':'
                {
                    decoded[1..].to_string()
                } else {
                    decoded
                }
            };

            // 1. Canonicalize first so symlinks are fully resolved before any check.
            //    This eliminates the TOCTOU window between check and read.
            let path = std::path::Path::new(&decoded);
            let canonical = match path.canonicalize() {
                Ok(c) => c,
                Err(_) => {
                    return tauri::http::Response::builder()
                        .status(tauri::http::StatusCode::NOT_FOUND)
                        .body(Vec::new())
                        .unwrap();
                }
            };

            // 2. Verify the canonical path is inside the vault root.
            //    Checking for `..` in the raw path is insufficient because a
            //    symlink (e.g. /vault/images/foo -> /etc) contains no `..` but
            //    escapes the vault after resolution.
            let vault_root = {
                let state = _app.app_handle().state::<VaultPathState>();
                let x = match state.0.lock() {
                    Ok(vp) if !vp.is_empty() => vp.as_str().to_owned(),
                    _ => String::new(),
                };
                x
            };

            if vault_root.is_empty() {
                return tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::FORBIDDEN)
                    .body(Vec::new())
                    .unwrap();
            }

            let canonical_vault = match std::path::Path::new(&vault_root).canonicalize() {
                Ok(c) => c,
                Err(_) => {
                    return tauri::http::Response::builder()
                        .status(tauri::http::StatusCode::FORBIDDEN)
                        .body(Vec::new())
                        .unwrap();
                }
            };

            if !canonical.starts_with(&canonical_vault) {
                return tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::FORBIDDEN)
                    .body(Vec::new())
                    .unwrap();
            }

            match std::fs::read(&canonical) {
                Ok(data) => {
                    let mime = mime_from_ext(&decoded);
                    tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(data)
                        .unwrap()
                }
                Err(_) => tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap(),
            }
        })
        .setup(|app| {
            // Create the main window with on_navigation to block external navigations
            use tauri::WebviewUrl;
            use tauri::WebviewWindowBuilder;
            let _win = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Margin")
                .inner_size(800.0, 600.0)
                .on_navigation(|url: &tauri::Url| {
                    let s = url.as_str();
                    s.starts_with("http://localhost")
                        || s.starts_with("https://tauri.localhost")
                        || s.starts_with("http://tauri.localhost")
                        || s.starts_with("tauri://")
                })
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crypto::generate_mnemonic,
            crypto::derive_vault_keys,
            crypto::encrypt_blob_cmd,
            crypto::decrypt_blob_cmd,
            fs::set_vault_directory,
            fs::read_file_bytes,
            fs::write_file_bytes,
            fs::list_directory,
            fs::walk_directory,
            fs::read_link_batch,
            fs::build_visible_tree,
            fs::build_subtree,
            fs::delete_entry,
            fs::rename_entry,
            fs::create_directory,
            fs::file_exists,
            fs::copy_file,
            fs::copy_directory,
            fs::reveal_in_file_manager,
            fs::set_mtime,
            fs::watch_file,
            fs::unwatch_file,
            fs::watch_vault,
            fs::unwatch_vault,
            fs::search_files,
            fs::search_file_contents,
            fs::replace_in_file,
            fs::list_all_tags,
            fs::export_vault_zip,
            fs::has_unsynced_changes,
            s3::s3_configure,
            s3::s3_get_config,
            s3::s3_test_connection,
            s3::s3_upload,
            s3::s3_download,
            s3::s3_list,
            s3::s3_delete,
            settings::save_settings,
            settings::load_settings,
            settings::export_settings_string,
            settings::import_settings_string,
            settings::save_workspace_state,
            settings::load_workspace_state,
            session::save_session,
            session::load_session,
            session::clear_session,
            session::load_vault_profiles,
            session::save_vault_profile,
            session::delete_vault_profile,
            history::save_snapshot,
            history::list_snapshots,
            history::read_snapshot,
            history::delete_snapshot,
            history::clear_snapshots,
            history::clear_history_tree,
            history::rename_history,
            text::search_in_text,
            text::extract_wiki_links,
            text_transform::fuzzy_filter_files,
            text_transform::transform_image_paths,
            sync::hash_files_batch,
            sync::load_manifest,
            sync::save_manifest,
            sync::compute_sync_actions,
            sync::collect_tombstones,
            sync::merge_tombstones,
            sync::prune_tombstones,
            sync::sync_upload_files,
            sync::sync_download_files,
            sync::sync_upload_manifest,
            themes::load_themes,
            themes::save_themes,
            themes::export_theme,
            themes::import_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
