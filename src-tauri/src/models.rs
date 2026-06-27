use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ── Daily Log ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DailyLog {
    pub id: i64,
    pub log_date: String,
    pub day_name: Option<String>,
    pub fatigue_desc: Option<String>,
    pub fatigue_rating: Option<f64>,
    pub headache_desc: Option<String>,
    pub headache_rating: Option<f64>,
    pub headache_duration_hours: Option<f64>,
    pub other_symptoms: Option<String>,
    pub my_sleep_rating: Option<f64>,
    pub phone_sleep_rating: Option<f64>,
    pub sleep_avg: Option<f64>,
    pub sleep_time_head_on_pillow: Option<f64>,
    pub sleep_actual_asleep: Option<f64>,
    pub sleep_rem: Option<f64>,
    pub sleep_deep: Option<f64>,
    pub sleep_awake: Option<f64>,
    pub steps: Option<i64>,
    pub activity_calories: Option<f64>,
    pub ave_resting_hr: Option<i64>,
    pub ave_hr: Option<i64>,
    pub hr_min: Option<i64>,
    pub hr_max: Option<i64>,
    pub rostered_hours: Option<f64>,
    pub sick_leave_hours: Option<f64>,
    pub office_hours: Option<f64>,
    pub wfh_hours: Option<f64>,
    pub alcohol_std_drinks: Option<f64>,
    pub multivitamin: Option<bool>,
    pub vitamin_c: Option<bool>,
    pub add_meds: Option<String>,
    pub compression_socks: Option<bool>,
    pub notes: Option<String>,
}

// ── Medications ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Medication {
    pub id: i64,
    pub name: String,
    pub short_code: Option<String>,
    pub default_dose: Option<f64>,
    pub dose_unit: Option<String>,
    pub category: Option<String>,
    pub active: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct MedicationDose {
    pub id: i64,
    pub medication_id: i64,
    pub log_date: String,
    pub time_taken: Option<String>,
    pub dose_amount: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct MedicationHistoryEntry {
    pub id: i64,
    pub medication_id: Option<i64>,
    pub medication_name: String,
    pub event_type: String,
    pub event_date: String,
    pub detail: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

// ── Watch calibration ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct WatchCalibration {
    pub id: i64,
    pub cal_date: String,
    pub cal_time: Option<String>,
    pub notes: Option<String>,
}

// ── Blood Pressure ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct BloodPressure {
    pub id: i64,
    pub log_date: String,
    pub reading_num: i64,
    pub time_taken: Option<String>,
    pub systolic: i64,
    pub diastolic: i64,
    pub notes: Option<String>,
}

// ── Activity ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityCategory {
    pub id: i64,
    pub name: String,
    pub energy_weight: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityType {
    pub id: i64,
    pub name: String,
    pub category_id: i64,
    pub default_energy_cost: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ActivityEntry {
    pub id: i64,
    pub log_date: String,
    pub activity_type_id: i64,
    pub duration_hours: f64,
    pub energy_cost: Option<String>,
    pub notes: Option<String>,
}

// ── PEM Model ──

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PemCalibration {
    pub id: i64,
    pub param_name: String,
    pub param_value: f64,
    pub description: Option<String>,
}

// PemPrediction is defined in commands/pem.rs

// ── AI: Ask & Insights ──

#[derive(Debug, Serialize, Deserialize)]
pub struct AskResponse {
    /// Plain-English answer for the user.
    pub answer: String,
    /// The read-only SQL the model generated and we executed ("the working").
    pub sql: String,
    /// What the query computes, in one line.
    pub explanation: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// True if more rows existed than we returned.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInsights {
    pub summary: String,
    pub patterns: Vec<InsightItem>,
    pub anomalies: Vec<InsightItem>,
    pub recommendations: Vec<InsightItem>,
    #[serde(default)]
    pub period_label: String,
    #[serde(default)]
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightItem {
    pub title: String,
    pub detail: String,
    pub severity: String,
    pub icon: String,
}