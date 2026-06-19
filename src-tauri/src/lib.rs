mod commands;
mod discord_rpc;
mod ffmpeg;
mod models;

use models::settings::WindowState;
use tauri::{Manager, WindowEvent};

const DISCORD_CLIENT_ID: &str = "1516792047095382087";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::rune_tags::seed_defaults();

    tauri::Builder::default()
        .setup(|app| {
            ffmpeg::init_paths(&app.handle());
            discord_rpc::connect(DISCORD_CLIENT_ID);
            discord_rpc::set_idle();

            // Apply saved window state after window is created
            let window = app.get_webview_window("main").unwrap();
            if let Some(state) = commands::load_window_state() {
                let _ = window.set_position(tauri::PhysicalPosition::new(state.x, state.y));
                let _ = window.set_size(tauri::PhysicalSize::new(state.width, state.height));
                if state.maximized {
                    let _ = window.maximize();
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::start_conversion,
            commands::get_media_info,
            commands::detect_ffmpeg,
            commands::get_default_output_dir,
            commands::scan_directory,
            commands::start_batch_conversion,
            commands::is_directory,
            commands::cancel_conversion,
            commands::estimate_compress_size,
            commands::extract_frames,
            commands::read_image_data_url,
            commands::list_rune_tags,
            commands::save_rune_tag,
            commands::delete_rune_tag,
            commands::apply_rune_tag,
            commands::update_discord_presence,
            commands::update_forge_presence,
            commands::set_discord_enabled,
            commands::pre_render_timeline,
            commands::delete_temp_file,
            commands::export_timeline,
            commands::cancel_forge_export,
            commands::save_project_file,
            commands::load_project_file,
            commands::read_file_bytes,
            commands::load_settings,
            commands::save_settings,
            commands::load_window_state,
            commands::save_window_state,
            commands::save_forge_recovery,
            commands::load_forge_recovery,
            commands::clear_forge_recovery,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                // Save window state on close
                if let Ok(position) = window.outer_position() {
                    if let Ok(size) = window.outer_size() {
                        let maximized = window.is_maximized().unwrap_or(false);
                        let state = WindowState {
                            x: position.x,
                            y: position.y,
                            width: size.width,
                            height: size.height,
                            maximized,
                        };
                        let _ = commands::save_window_state(state);
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}