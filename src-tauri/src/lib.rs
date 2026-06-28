mod commands;
mod db;
mod models;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let pool = tauri::async_runtime::block_on(db::init_db());
            app.manage(pool);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::daily_log::get_daily_log,
            commands::daily_log::upsert_daily_log,
            commands::daily_log::list_daily_logs,
            commands::medications::list_medications,
            commands::medications::create_medication,
            commands::medications::update_medication,
            commands::medications::archive_medication,
            commands::medications::delete_medication,
            commands::medications::get_medication_schedule,
            commands::medications::add_schedule_item,
            commands::medications::update_schedule_item,
            commands::medications::delete_schedule_item,
            commands::medications::get_doses_for_date,
            commands::medications::upsert_dose,
            commands::medications::delete_dose,
            commands::medications::get_medication_history,
            commands::medications::add_medication_history,
            commands::medications::update_medication_history,
            commands::medications::delete_medication_history,
            commands::watch_calibration::log_watch_calibration,
            commands::watch_calibration::list_watch_calibrations,
            commands::watch_calibration::delete_watch_calibration,
            commands::watch_calibration::days_since_calibration,
            commands::blood_pressure::get_bp_for_date,
            commands::blood_pressure::get_bp_history,
            commands::blood_pressure::upsert_bp,
            commands::blood_pressure::delete_bp,
            commands::activity::list_activity_categories,
            commands::activity::list_activity_types,
            commands::activity::get_activities_for_date,
            commands::activity::add_activity_entry,
            commands::activity::set_activity_duration,
            commands::activity::delete_activity_entry,
            commands::pem::get_calibration_params,
            commands::pem::update_calibration_param,
            commands::pem::get_pem_predictions,
            commands::pem::run_pem_model,
            commands::dashboard::get_dashboard_summary,
            commands::import_xlsx::import_spreadsheet,
            commands::export::export_csv,
            commands::export::export_json,
            commands::settings::save_api_key,
            commands::settings::get_api_key,
            commands::settings::get_ai_model,
            commands::settings::save_ai_model,
            commands::ask::ask_question,
            commands::insights::get_insights,
            commands::insights::refresh_insights,
            commands::settings::get_sync_settings,
            commands::settings::save_sync_settings,
            commands::settings::get_app_prefs,
            commands::settings::save_app_prefs,
            commands::csv_import::import_health_csv,
            commands::update::latest_build_commit,
            commands::vault::get_vault_index,
            commands::vault::read_vault_note,
            commands::settings::get_vault_settings,
            commands::settings::save_vault_settings,
            commands::labs::extract_lab_results,
            commands::labs::get_lab_tests,
            commands::labs::get_lab_series,
            commands::labs::get_labs_last_extract,
            commands::records_ask::ask_records,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}