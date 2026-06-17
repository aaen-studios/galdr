mod commands;
mod ffmpeg;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::rune_tags::seed_defaults();

    tauri::Builder::default()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
