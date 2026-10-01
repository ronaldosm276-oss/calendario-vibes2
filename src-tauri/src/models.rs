use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CalendarInfo {
    pub id: String,
    pub summary: String,
    pub description: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
    #[serde(rename = "foregroundColor")]
    pub foreground_color: Option<String>,
    pub primary: Option<bool>,
    #[serde(rename = "accessRole")]
    pub access_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventTime {
    pub date: Option<String>,
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalendarEvent {
    pub id: String,
    #[serde(rename = "calendarId")]
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "colorId")]
    pub color_id: Option<String>,
    #[serde(rename = "eventColor")]
    pub event_color: Option<String>,
    #[serde(rename = "calendarColor")]
    pub calendar_color: Option<String>,
    #[serde(rename = "htmlLink")]
    pub html_link: Option<String>,
    pub start: EventTime,
    pub end: EventTime,
    #[serde(rename = "recurringEventId")]
    pub recurring_event_id: Option<String>,
    #[serde(rename = "originalStartTime")]
    pub original_start_time: Option<EventTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReminderRule {
    #[serde(default)]
    pub minutes_before: i64,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlarmInfo {
    pub id: String,
    pub event_key: String,
    pub calendar_id: String,
    pub event_id: String,
    pub title: String,
    pub start_at: String,
    pub sound: Option<String>,
    pub snooze_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoundInfo {
    pub name: String,
    pub path: String,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,
    #[serde(alias = "always_on_top")]
    pub always_on_top: bool,
    #[serde(alias = "start_with_windows")]
    pub start_with_windows: bool,
    #[serde(alias = "start_hidden")]
    pub start_hidden: bool,
    #[serde(alias = "remember_position")]
    pub remember_position: bool,
    #[serde(alias = "remember_size")]
    pub remember_size: bool,
    pub opacity: f32,
    #[serde(alias = "default_calendar_id")]
    pub default_calendar_id: Option<String>,
    #[serde(alias = "visible_calendar_ids")]
    pub visible_calendar_ids: Vec<String>,
    #[serde(alias = "first_day_of_week")]
    pub first_day_of_week: u8,
    pub time_format: String,
    #[serde(alias = "automatic_sync")]
    pub automatic_sync: bool,
    #[serde(alias = "sync_interval_minutes")]
    pub sync_interval_minutes: u32,
    #[serde(alias = "selected_month")]
    pub selected_month: String,
    pub calendar_colors: HashMap<String, String>,
    pub event_colors: HashMap<String, String>,
    pub font_scale: f64,
    pub density: String,
    pub zoom: f64,
    pub accent_color: Option<String>,
    pub reminders_enabled: bool,
    pub default_reminder_enabled: bool,
    pub default_reminder_minutes: i64,
    pub default_reminder_sound: Option<String>,
    pub alarm_volume: f32,
    pub snooze_minutes: u32,
    pub reminders: HashMap<String, Vec<ReminderRule>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            always_on_top: false,
            start_with_windows: false,
            start_hidden: false,
            remember_position: true,
            remember_size: true,
            opacity: 0.96,
            default_calendar_id: None,
            visible_calendar_ids: vec![],
            first_day_of_week: 1,
            time_format: "24h".into(),
            automatic_sync: true,
            sync_interval_minutes: 15,
            selected_month: "".into(),
            calendar_colors: HashMap::new(),
            event_colors: HashMap::new(),
            font_scale: 1.0,
            density: "comfortable".into(),
            zoom: 1.0,
            accent_color: None,
            reminders_enabled: true,
            default_reminder_enabled: true,
            default_reminder_minutes: 10,
            default_reminder_sound: None,
            alarm_volume: 0.9,
            snooze_minutes: 5,
            reminders: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthState {
    pub authenticated: bool,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub status: String,
    #[serde(alias = "last_synced_at")]
    pub last_synced_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}
