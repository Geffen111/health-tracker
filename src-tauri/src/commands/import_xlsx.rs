use calamine::{Data, DataType, open_workbook, Reader, Xlsx};
use chrono::NaiveDate;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub async fn import_spreadsheet(
    pool: State<'_, SqlitePool>,
    file_path: String,
) -> Result<String, String> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let mut workbook: Xlsx<_> = open_workbook(path).map_err(|e| format!("Failed to open XLSX: {}", e))?;

    let mut total_daily = 0;
    let mut total_activities = 0;
    let mut total_calibration = 0;
    let mut errors: Vec<String> = Vec::new();

    // ── Sheet 1: Fatigue Log ──
    if let Ok(range) = workbook.worksheet_range("Fatigue Log") {
        let all_rows: Vec<Vec<calamine::Data>> = range.rows().map(|r| r.to_vec()).collect();
        let mut iter = all_rows.iter();
        let _header = iter.next();
        let _subheader = iter.next();

        for (i, row) in iter.enumerate() {
            if row.is_empty() || row.iter().all(|c| c.is_empty()) || date_cell_empty(row) {
                continue;
            }
            match import_daily_row(&pool, row).await {
                Ok(_) => total_daily += 1,
                Err(e) => errors.push(format!("Daily row {}: {}", i + 1, e)),
            }
        }
    } else {
        errors.push("Sheet 'Fatigue Log' not found".to_string());
    }

    // ── Sheet 2: ActivityLog ──
    if let Ok(range) = workbook.worksheet_range("ActivityLog") {
        // activity_log has no natural unique key (a day can repeat the same activity),
        // so a plain re-import would duplicate. Clear it first to keep import idempotent.
        let _ = sqlx::query("DELETE FROM activity_log").execute(&*pool).await;

        let all_rows: Vec<Vec<calamine::Data>> = range.rows().map(|r| r.to_vec()).collect();
        let mut iter = all_rows.iter();
        let _header = iter.next();
        for (i, row) in iter.enumerate() {
            // Skip blank/trailing rows (used range extends well past the real data).
            if row.is_empty() || row.iter().all(|c| c.is_empty()) || date_cell_empty(row) {
                continue;
            }
            match import_activity_row(&pool, row).await {
                Ok(_) => total_activities += 1,
                Err(e) => errors.push(format!("Activity row {}: {}", i + 1, e)),
            }
        }
    }

    // ── Sheet 3: Calibration ──
    if let Ok(range) = workbook.worksheet_range("Calibration") {
        for raw_row in range.rows() {
            let row: Vec<calamine::Data> = raw_row.to_vec();
            if row.len() < 2 {
                continue;
            }
            let param_name = match &row[0] {
                Data::String(s) => s.trim().to_string(),
                _ => continue,
            };
            if param_name.is_empty() || param_name == "Parameter" {
                continue;
            }
            if let Some(val) = row[1].get_float() {
                let desc = row.get(2)
                    .map(|c| match c { Data::String(s) => s.clone(), _ => c.to_string() })
                    .unwrap_or_default();
                let _ = sqlx::query(
                    "INSERT OR REPLACE INTO pem_calibration (param_name, param_value, description) VALUES (?, ?, ?)"
                )
                .bind(&param_name).bind(val).bind(&desc)
                .execute(&*pool).await;
                total_calibration += 1;
            }
        }
    }

    // Record the import
    sqlx::query(
        "INSERT INTO import_log (source, filename, rows_imported, rows_skipped)
         VALUES ('xlsx_import', ?, ?, ?)"
    )
    .bind(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
    .bind(total_daily + total_activities + total_calibration)
    .bind(errors.len() as i64)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let summary = format!(
        "✅ Imported {} daily logs, {} activities, {} calibration params. {} errors.",
        total_daily, total_activities, total_calibration, errors.len()
    );

    if errors.is_empty() {
        Ok(summary)
    } else {
        let err_detail: String = errors.into_iter().take(5).collect::<Vec<_>>().join("\n");
        Ok(format!("{}\nWarnings:\n{}", summary, err_detail))
    }
}

async fn import_daily_row(pool: &SqlitePool, row: &[Data]) -> Result<(), String> {
    // Column mapping:
    // 0=Date, 1=Day, 2=My Sleep, 3=Phone Sleep, 4=Sleep Ave,
    // 5=Fatigue(desc), 6=Fatigue Rating, 7=Headache(desc), 8=H/A Rating, 9=Duration(h),
    // 10=Other Symptoms,
    // 11-16=Dex 1-3 (time/dose pairs: 11=Dex1_time,12=Dex1_dose,13=Dex2_time,14=Dex2_dose,15=Dex3_time,16=Dex3_dose),
    // 17=Esc time, 18=Esc dose, 19=Cand time, 20=Cand dose,
    // 21=Ago time, 22=Ago dose, 23=Pant time, 24=Pant dose,
    // 25=Mel time, 26=Mel dose, 27=Multivitamin, 28=Vitamin C, 29=Add Meds,
    // 30=BP1_time, 31=BP1_sys, 32=BP1_dia, 33=BP2_time, 34=BP2_sys, 35=BP2_dia,
    // 36=BP3_time, 37=BP3_sys, 38=BP3_dia, 39=Sys Ave, 40=Dia Ave, 41=BP Note,
    // 42=Ave Resting HR, 43=Ave H/R, 44=Steps, 45=Activity(kCal),
    // 46=Rostered Hours, 47=Sick Leave, 48=Office Hours, 49=WFH Hours,
    // 50=Alcohol, 51=Notes,
    // 52=Sleep(head on pillow), 53=Sleep(actual), 54=REM, 55=Deep, 56=Awake

    let date_str = get_date(row, 0)?;
    let day_name = cell_string(row.get(1));

    // Sleep
    let my_sleep = row.get(2).and_then(|c| c.get_float());
    let phone_sleep = row.get(3).and_then(|c| c.get_float());
    let sleep_avg = row.get(4).and_then(|c| c.get_float());

    // Fatigue
    let fatigue_desc = cell_string(row.get(5));
    let fatigue_rating = row.get(6).and_then(|c| c.get_float());

    // Headache
    let headache_desc = cell_string(row.get(7));
    let headache_rating = row.get(8).and_then(|c| c.get_float());
    let headache_duration = row.get(9).and_then(|c| c.get_float());

    // Other symptoms
    let other_symptoms = cell_string(row.get(10));

    // Steps & activity
    let steps = row.get(44).and_then(|c| c.get_float()).map(|f| f as i64);
    let activity_calories = row.get(45).and_then(|c| c.get_float());
    let resting_hr = row.get(42).and_then(|c| c.get_float()).map(|f| f as i64);
    let avg_hr = row.get(43).and_then(|c| c.get_float()).map(|f| f as i64);

    // Work hours
    let rostered = row.get(46).and_then(|c| c.get_float());
    let sick = row.get(47).and_then(|c| c.get_float());
    let office = row.get(48).and_then(|c| c.get_float());
    let wfh = row.get(49).and_then(|c| c.get_float());
    let alcohol = row.get(50).and_then(|c| c.get_float());

    // Supplements
    let multivitamin = cell_bool(row.get(27));
    let vitamin_c = cell_bool(row.get(28));
    let add_meds = cell_string(row.get(29));
    let notes = cell_string(row.get(51));

    // Sleep stages
    let sleep_pillow = row.get(52).and_then(|c| c.get_float());
    let sleep_asleep = row.get(53).and_then(|c| c.get_float());
    let sleep_rem = row.get(54).and_then(|c| c.get_float());
    let sleep_deep = row.get(55).and_then(|c| c.get_float());
    let sleep_awake = row.get(56).and_then(|c| c.get_float());

    // Upsert daily log
    sqlx::query(
        "INSERT INTO daily_logs (log_date, day_name, fatigue_desc, fatigue_rating,
         headache_desc, headache_rating, headache_duration_hours, other_symptoms,
         my_sleep_rating, phone_sleep_rating, sleep_avg,
         sleep_time_head_on_pillow, sleep_actual_asleep, sleep_rem, sleep_deep, sleep_awake,
         steps, activity_calories, ave_resting_hr, ave_hr,
         rostered_hours, sick_leave_hours, office_hours, wfh_hours, alcohol_std_drinks,
         multivitamin, vitamin_c, add_meds, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(log_date) DO UPDATE SET
         fatigue_desc=excluded.fatigue_desc, fatigue_rating=excluded.fatigue_rating,
         headache_desc=excluded.headache_desc, headache_rating=excluded.headache_rating,
         other_symptoms=excluded.other_symptoms, notes=excluded.notes"
    )
    .bind(&date_str).bind(&day_name).bind(&fatigue_desc).bind(fatigue_rating)
    .bind(&headache_desc).bind(headache_rating).bind(headache_duration)
    .bind(&other_symptoms).bind(my_sleep).bind(phone_sleep).bind(sleep_avg)
    .bind(sleep_pillow).bind(sleep_asleep).bind(sleep_rem).bind(sleep_deep).bind(sleep_awake)
    .bind(steps).bind(activity_calories).bind(resting_hr).bind(avg_hr)
    .bind(rostered).bind(sick).bind(office).bind(wfh).bind(alcohol)
    .bind(multivitamin).bind(vitamin_c).bind(&add_meds).bind(&notes)
    .execute(pool).await.map_err(|e| format!("DB insert: {}", e))?;

    // Import blood pressure readings (3x)
    let bp_groups = [
        (1, row.get(30), row.get(31), row.get(32)),
        (2, row.get(33), row.get(34), row.get(35)),
        (3, row.get(36), row.get(37), row.get(38)),
    ];

    for (reading_num, time_cell, sys_cell, dia_cell) in bp_groups {
        if let (Some(sys), Some(dia)) = (sys_cell.and_then(|c| c.get_float()), dia_cell.and_then(|c| c.get_float())) {
            let bp_time = cell_time_string(time_cell);
            let _ = sqlx::query(
                "INSERT INTO blood_pressure (log_date, reading_num, time_taken, systolic, diastolic)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(log_date, reading_num) DO UPDATE SET
                 systolic=excluded.systolic, diastolic=excluded.diastolic"
            )
            .bind(&date_str).bind(reading_num).bind(&bp_time).bind(sys as i64).bind(dia as i64)
            .execute(pool).await;
        }
    }

    // Import medication doses from columns 11-26
    let med_map: [(usize, usize, &str); 8] = [
        (11, 12, "Dexamphetamine"),
        (13, 14, "Dexamphetamine"),
        (15, 16, "Dexamphetamine"),
        (17, 18, "Escitalopram"),
        (19, 20, "Candesartan"),
        (21, 22, "Agomelatine"),
        (23, 24, "Pantoprazole"),
        (25, 26, "Melatonin"),
    ];

    for (time_col, dose_col, med_name) in med_map {
        // The dose is the source of truth that a dose was taken; the time is a
        // time-formatted cell (calamine returns it as DateTime/float, not text) and
        // is optional. Import whenever a dose amount is present.
        let dose_val = row.get(dose_col).and_then(|c| c.get_float());
        if let Some(dose) = dose_val {
            let time = cell_time_string(row.get(time_col)).unwrap_or_default();
            let med_id = find_or_create_medication(pool, med_name).await?;
            let _ = sqlx::query(
                "INSERT INTO medication_doses (medication_id, log_date, time_taken, dose_amount)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(medication_id, log_date, time_taken) DO UPDATE SET dose_amount=excluded.dose_amount"
            )
            .bind(med_id).bind(&date_str).bind(&time).bind(dose)
            .execute(pool).await;
        }
    }

    Ok(())
}

async fn import_activity_row(pool: &SqlitePool, row: &[Data]) -> Result<(), String> {
    let date_str = get_date(row, 0)?;
    let activity_name = cell_string(row.get(1)).unwrap_or_default();
    if activity_name.is_empty() {
        return Ok(());
    }
    let duration = row.get(2).and_then(|c| c.get_float()).unwrap_or(0.0);
    let energy_cost = cell_string(row.get(4));

    let activity_type: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM activity_types WHERE name = ?"
    )
    .bind(&activity_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Activity lookup: {}", e))?;

    if let Some((type_id,)) = activity_type {
        let _ = sqlx::query(
            "INSERT INTO activity_log (log_date, activity_type_id, duration_hours, energy_cost)
             VALUES (?, ?, ?, ?)"
        )
        .bind(&date_str).bind(type_id).bind(duration).bind(&energy_cost)
        .execute(pool).await;
    }

    Ok(())
}

async fn find_or_create_medication(pool: &SqlitePool, name: &str) -> Result<i64, String> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM medications WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    // Create the medication
    let short_code = match name {
        "Dexamphetamine" => Some("Dex"),
        "Escitalopram" => Some("Esc"),
        "Candesartan" => Some("Cand"),
        "Agomelatine" => Some("Ago"),
        "Pantoprazole" => Some("Pant"),
        "Melatonin" => Some("Mel"),
        _ => None,
    };

    sqlx::query(
        "INSERT INTO medications (name, short_code, default_dose, dose_unit, category)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(name).bind(&short_code.map(|s| s.to_string()))
    .bind(match name {
        "Dexamphetamine" => Some(10.0),
        "Escitalopram" => Some(5.0),
        "Candesartan" => Some(4.0),
        "Agomelatine" => Some(25.0),
        "Pantoprazole" => Some(20.0),
        "Melatonin" => Some(2.0),
        _ => None,
    })
    .bind(Some("mg".to_string()))
    .bind(match name {
        "Dexamphetamine" => Some("stimulant"),
        "Escitalopram" => Some("antidepressant"),
        "Candesartan" => Some("BP"),
        "Agomelatine" => Some("antidepressant"),
        "Pantoprazole" => Some("GI"),
        "Melatonin" => Some("sleep"),
        _ => None,
    }.map(|s| s.to_string()))
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let new_id: (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    Ok(new_id.0)
}

/// Convert Excel cell to YYYY-MM-DD date string
fn get_date(row: &[Data], col: usize) -> Result<String, String> {
    let cell = row.get(col).ok_or_else(|| format!("Missing column {}", col))?;

    // Excel serial date — may arrive as a plain Float OR as a typed DateTime cell
    // (calamine surfaces date-formatted cells as Data::DateTime, whose get_float() is None).
    let serial_opt = match cell {
        Data::DateTime(dt) => Some(dt.as_f64()),
        _ => cell.get_float(),
    };
    if let Some(serial) = serial_opt {
        let serial_i64 = serial as i64;
        let epoch = NaiveDate::from_ymd_opt(1899, 12, 30)
            .ok_or("Invalid epoch date")?;
        let date = epoch.checked_add_signed(chrono::Duration::days(serial_i64))
            .ok_or("Date overflow")?;
        return Ok(date.format("%Y-%m-%d").to_string());
    }

    // Try string
    let s = match cell {
        Data::String(s) => s.trim().to_string(),
        _ => return Err(format!("Cannot parse date from: {:?}", cell)),
    };
    if s.is_empty() {
        return Err("Empty date cell".to_string());
    }

    // Try common formats
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%d/%m/%Y") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%d/%m/%y") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }

    Err(format!("Cannot parse date: '{}'", s))
}

/// Extract a non-empty string from an optional Data cell
fn cell_string(cell: Option<&Data>) -> Option<String> {
    cell.and_then(|c| match c {
        Data::String(s) => {
            let cleaned = s.trim().to_string();
            if cleaned.is_empty() { None } else { Some(cleaned) }
        }
        _ => None,
    })
}

/// True when the date cell (column 0) is missing/blank — used to skip the trailing
/// empty rows in a sheet's used range without flagging them as errors.
fn date_cell_empty(row: &[Data]) -> bool {
    match row.get(0) {
        None | Some(Data::Empty) => true,
        Some(Data::String(s)) => s.trim().is_empty(),
        _ => false,
    }
}

/// Render a time-of-day cell as "HH:MM". Excel stores times as a fraction of a day,
/// which calamine surfaces as Data::Float or a typed Data::DateTime — neither of which
/// `cell_string` can read. Already-text cells pass through.
fn cell_time_string(cell: Option<&Data>) -> Option<String> {
    match cell? {
        Data::String(s) => {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        }
        other => {
            let serial = match other {
                Data::DateTime(dt) => Some(dt.as_f64()),
                _ => other.get_float(),
            }?;
            let frac = serial - serial.floor();
            let total_min = (frac * 24.0 * 60.0).round() as i64;
            Some(format!("{:02}:{:02}", (total_min / 60) % 24, total_min % 60))
        }
    }
}

/// Check if a cell has a truthy value (for boolean columns like Multivitamin)
fn cell_bool(cell: Option<&Data>) -> bool {
    match cell {
        Some(Data::String(s)) => !s.trim().is_empty(),
        Some(Data::Float(f)) => *f > 0.0,
        Some(Data::Int(i)) => *i > 0,
        _ => false,
    }
}

// Helper trait removed — calamine::Data has .get_float() natively via DataType