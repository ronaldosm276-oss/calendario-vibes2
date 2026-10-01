use crate::{audio::AudioController, error::AppError, models::{AlarmInfo, CalendarEvent, ReminderRule}, storage};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{Arc, Mutex}, time::Duration};
use tauri::{Emitter, Manager};

#[derive(Clone)]
pub struct ReminderService {
    state: Arc<Mutex<ReminderState>>,
    audio: AudioController,
}

#[derive(Default)]
struct ReminderState {
    active: HashMap<String, AlarmInfo>,
    fired: HashSet<String>,
    snoozed: HashMap<String, (AlarmInfo, DateTime<Utc>)>,
}

impl ReminderService {
    pub fn new(audio: AudioController) -> Self { Self { state: Arc::new(Mutex::new(ReminderState::default())), audio } }
    pub fn active(&self) -> Vec<AlarmInfo> { let mut values: Vec<_> = self.state.lock().unwrap().active.values().cloned().collect(); values.sort_by(|a,b| a.start_at.cmp(&b.start_at)); values }

    pub fn acknowledge(&self, id: &str) -> Result<(), AppError> {
        self.state.lock().unwrap().active.remove(id);
        if self.state.lock().unwrap().active.is_empty() { self.audio.stop()?; }
        Ok(())
    }

    pub fn snooze(&self, id: &str, minutes: u32) -> Result<(), AppError> {
        let alarm = self.state.lock().unwrap().active.remove(id);
        if let Some(alarm) = alarm {
            let until = Utc::now() + chrono::Duration::minutes(minutes.max(1) as i64);
            self.state.lock().unwrap().snoozed.insert(id.to_string(), (alarm, until));
        }
        if self.state.lock().unwrap().active.is_empty() { self.audio.stop()?; }
        Ok(())
    }

    fn due_snoozes(&self, now: DateTime<Utc>) -> Vec<AlarmInfo> {
        let mut state = self.state.lock().unwrap();
        let ids: Vec<String> = state.snoozed.iter().filter_map(|(id, (_, when))| if *when <= now { Some(id.clone()) } else { None }).collect();
        ids.into_iter().filter_map(|id| state.snoozed.remove(&id).map(|(alarm, _)| alarm)).collect()
    }

    fn trigger(&self, app: &tauri::AppHandle, event: &CalendarEvent, rule: &ReminderRule, reminder_index: usize) -> Result<(), AppError> {
        let start = event_start_utc(event)?;
        let event_key = format!("{}:{}", event.calendar_id, event.id);
        let id = format!("{}:{}:{}", event_key, reminder_index, start.timestamp());
        {
            let mut state = self.state.lock().unwrap();
            if state.fired.contains(&id) || state.active.contains_key(&id) { return Ok(()); }
            state.fired.insert(id.clone());
            state.active.insert(id.clone(), AlarmInfo {
                id: id.clone(), event_key: event_key.clone(), calendar_id: event.calendar_id.clone(), event_id: event.id.clone(),
                title: event.summary.clone(), start_at: start.to_rfc3339(), sound: rule.sound.clone(),
                snooze_minutes: storage::get_settings(app)?.snooze_minutes,
            });
        }
        let settings = storage::get_settings(app)?;
        let path = match resolve_sound_path(app, rule.sound.as_deref()) {
            Ok(path) => path,
            Err(error) => { log::warn!("Som do lembrete indisponível: {error}"); None }
        };
        if let Err(error) = self.audio.play(path, settings.alarm_volume, true) {
            log::warn!("Não foi possível tocar o som do lembrete: {error}");
        }
        let alarm = self.state.lock().unwrap().active.get(&id).cloned().unwrap();
        show_alarm_window(app);
        app.emit("reminder-triggered", alarm).map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }
}

pub fn resolve_sound_path(app: &tauri::AppHandle, name: Option<&str>) -> Result<Option<PathBuf>, AppError> {
    let Some(name) = name.filter(|value| !value.trim().is_empty()) else { return Ok(None); };
    let dir = storage::sounds_dir(app)?;
    let candidate = dir.join(name);
    if candidate.parent() != Some(dir.as_path()) || !candidate.exists() { return Err(AppError::InvalidData("Arquivo de som não encontrado.".into())); }
    Ok(Some(candidate))
}

fn show_alarm_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("alarm") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn start_scheduler(app: tauri::AppHandle, service: ReminderService) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(error) = tick(&app, &service) { log::warn!("Reminder scheduler: {error}"); }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

fn tick(app: &tauri::AppHandle, service: &ReminderService) -> Result<(), AppError> {
    let now = Utc::now();
    for alarm in service.due_snoozes(now) {
        let settings = storage::get_settings(app)?;
        let mut state = service.state.lock().unwrap();
        state.active.insert(alarm.id.clone(), alarm.clone());
        drop(state);
        let path = match resolve_sound_path(app, alarm.sound.as_deref()) {
            Ok(path) => path,
            Err(error) => { log::warn!("Som do lembrete indisponível: {error}"); None }
        };
        if let Err(error) = service.audio.play(path, settings.alarm_volume, true) {
            log::warn!("Não foi possível tocar o som do lembrete: {error}");
        }
        show_alarm_window(app);
        app.emit("reminder-triggered", alarm).map_err(|e| AppError::Storage(e.to_string()))?;
    }

    let settings = storage::get_settings(app)?;
    if !settings.reminders_enabled { return Ok(()); }
    let events = storage::all_cached_events(app)?;
    for event in events {
        let key = format!("{}:{}", event.calendar_id, event.id);
        let Some(rules) = settings.reminders.get(&key) else { continue; };
        let Some(start) = event_start_utc(&event).ok() else { continue; };
        for (index, rule) in rules.iter().enumerate() {
            if !rule.enabled || rule.minutes_before < 0 { continue; }
            let due = start - chrono::Duration::minutes(rule.minutes_before);
            if due <= now && now - due <= chrono::Duration::minutes(10) {
                service.trigger(app, &event, rule, index)?;
            }
        }
    }
    Ok(())
}

fn event_start_utc(event: &CalendarEvent) -> Result<DateTime<Utc>, AppError> {
    if let Some(value) = event.start.date_time.as_deref() {
        return DateTime::parse_from_rfc3339(value)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::InvalidData(format!("Data do evento inválida: {e}")));
    }
    if let Some(value) = event.start.date.as_deref() {
        let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|e| AppError::InvalidData(format!("Data do evento inválida: {e}")))?;
        let local = Local.from_local_datetime(&date.and_hms_opt(0,0,0).unwrap()).single()
            .ok_or_else(|| AppError::InvalidData("Horário local ambíguo.".into()))?;
        return Ok(local.with_timezone(&Utc));
    }
    Err(AppError::InvalidData("Evento sem horário inicial.".into()))
}
