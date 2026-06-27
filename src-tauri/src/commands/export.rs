use crate::db::get_data_dir;
use crate::models::{
    ActivityCategory, ActivityEntry, ActivityType, BloodPressure, DailyLog, Medication,
    MedicationDose, MedicationHistoryEntry, PemCalibration, WatchCalibration,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tauri::State;

/// All source tables, fetched once and reused by both exporters. Derived data
/// (pem_predictions) is regenerable via the PEM model, so it's not exported.
struct Tables {
    daily_logs: Vec<DailyLog>,
    medications: Vec<Medication>,
    medication_doses: Vec<MedicationDose>,
    medication_history: Vec<MedicationHistoryEntry>,
    blood_pressure: Vec<BloodPressure>,
    activity_categories: Vec<ActivityCategory>,
    activity_types: Vec<ActivityType>,
    activity_log: Vec<ActivityEntry>,
    watch_calibration: Vec<WatchCalibration>,
    pem_calibration: Vec<PemCalibration>,
}

async fn fetch_all(pool: &SqlitePool) -> Result<Tables, String> {
    async fn q<T>(pool: &SqlitePool, sql: &str) -> Result<Vec<T>, String>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        sqlx::query_as::<_, T>(sql)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())
    }

    Ok(Tables {
        daily_logs: q(pool, "SELECT * FROM daily_logs ORDER BY log_date").await?,
        medications: q(pool, "SELECT * FROM medications ORDER BY id").await?,
        medication_doses: q(pool, "SELECT * FROM medication_doses ORDER BY log_date, id").await?,
        medication_history: q(pool, "SELECT * FROM medication_history ORDER BY event_date, id").await?,
        blood_pressure: q(pool, "SELECT * FROM blood_pressure ORDER BY log_date, reading_num").await?,
        activity_categories: q(pool, "SELECT * FROM activity_categories ORDER BY id").await?,
        activity_types: q(pool, "SELECT * FROM activity_types ORDER BY id").await?,
        activity_log: q(pool, "SELECT * FROM activity_log ORDER BY log_date, id").await?,
        watch_calibration: q(pool, "SELECT * FROM watch_calibration ORDER BY cal_date, id").await?,
        pem_calibration: q(pool, "SELECT * FROM pem_calibration ORDER BY id").await?,
    })
}

fn exports_root() -> Result<PathBuf, String> {
    let root = get_data_dir().join("exports");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Export every table as a separate CSV inside a timestamped folder. Returns the
/// folder path.
#[tauri::command]
pub async fn export_csv(pool: State<'_, SqlitePool>) -> Result<String, String> {
    let t = fetch_all(&pool).await?;
    let dir = exports_root()?.join(format!("health-export-{}", timestamp()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    fn write<T: Serialize>(dir: &Path, name: &str, rows: &[T]) -> Result<(), String> {
        let mut wtr = csv::Writer::from_path(dir.join(name)).map_err(|e| e.to_string())?;
        for row in rows {
            wtr.serialize(row).map_err(|e| e.to_string())?;
        }
        wtr.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    write(&dir, "daily_logs.csv", &t.daily_logs)?;
    write(&dir, "medications.csv", &t.medications)?;
    write(&dir, "medication_doses.csv", &t.medication_doses)?;
    write(&dir, "medication_history.csv", &t.medication_history)?;
    write(&dir, "blood_pressure.csv", &t.blood_pressure)?;
    write(&dir, "activity_categories.csv", &t.activity_categories)?;
    write(&dir, "activity_types.csv", &t.activity_types)?;
    write(&dir, "activity_log.csv", &t.activity_log)?;
    write(&dir, "watch_calibration.csv", &t.watch_calibration)?;
    write(&dir, "pem_calibration.csv", &t.pem_calibration)?;

    Ok(dir.to_string_lossy().to_string())
}

/// Export every table into a single pretty-printed JSON file. Returns the file path.
#[tauri::command]
pub async fn export_json(pool: State<'_, SqlitePool>) -> Result<String, String> {
    let t = fetch_all(&pool).await?;

    #[derive(Serialize)]
    struct FullExport<'a> {
        exported_at: String,
        daily_logs: &'a [DailyLog],
        medications: &'a [Medication],
        medication_doses: &'a [MedicationDose],
        medication_history: &'a [MedicationHistoryEntry],
        blood_pressure: &'a [BloodPressure],
        activity_categories: &'a [ActivityCategory],
        activity_types: &'a [ActivityType],
        activity_log: &'a [ActivityEntry],
        watch_calibration: &'a [WatchCalibration],
        pem_calibration: &'a [PemCalibration],
    }

    let export = FullExport {
        exported_at: chrono::Local::now().to_rfc3339(),
        daily_logs: &t.daily_logs,
        medications: &t.medications,
        medication_doses: &t.medication_doses,
        medication_history: &t.medication_history,
        blood_pressure: &t.blood_pressure,
        activity_categories: &t.activity_categories,
        activity_types: &t.activity_types,
        activity_log: &t.activity_log,
        watch_calibration: &t.watch_calibration,
        pem_calibration: &t.pem_calibration,
    };

    let json = serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?;
    let path = exports_root()?.join(format!("health-export-{}.json", timestamp()));
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
