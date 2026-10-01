use crate::{error::AppError, models::{CalendarEvent, Settings, SoundInfo, WindowState}};
use keyring::Entry;
use tauri::Manager;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

const SERVICE: &str = "calendar-gadget";
const REFRESH_TOKEN: &str = "google-refresh-token";

pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    app.path().app_data_dir().map_err(|e| AppError::Storage(e.to_string()))
}

pub fn sounds_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let dir = data_dir(app)?.join("sounds");
    fs::create_dir_all(&dir).map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(dir)
}

pub fn list_sounds(app: &tauri::AppHandle) -> Result<Vec<SoundInfo>, AppError> {
    let dir = sounds_dir(app)?;
    let mut sounds = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| AppError::Storage(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::Storage(e.to_string()))?;
        let path = entry.path();
        if !path.is_file() { continue; }
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(ext.as_str(), "wav" | "mp3") { continue; }
        let Some(name) = path.file_name().and_then(|x| x.to_str()) else { continue; };
        sounds.push(SoundInfo { name: name.into(), path: path.to_string_lossy().into_owned() });
    }
    sounds.sort_by_key(|sound| sound.name.to_ascii_lowercase());
    Ok(sounds)
}

pub fn all_cached_events(app: &tauri::AppHandle) -> Result<Vec<CalendarEvent>, AppError> {
    read_cache(app, |cache| cache.events.clone())
}

pub fn add_sound(app: &tauri::AppHandle, source: &Path) -> Result<SoundInfo, AppError> {
    let Some(name) = source.file_name().and_then(|value| value.to_str()) else {
        return Err(AppError::InvalidData("Nome de arquivo inválido.".into()));
    };
    let ext = source.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "wav" | "mp3") {
        return Err(AppError::InvalidData("Formato de áudio não suportado. Use WAV ou MP3.".into()));
    }
    let dir = sounds_dir(app)?;
    let destination = dir.join(name);
    fs::copy(source, &destination).map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(SoundInfo { name: name.into(), path: destination.to_string_lossy().into_owned() })
}

pub fn remove_sound(app: &tauri::AppHandle, name: &str) -> Result<(), AppError> {
    let dir = sounds_dir(app)?;
    let candidate = dir.join(name);
    if candidate.parent() != Some(dir.as_path()) {
        return Err(AppError::InvalidData("Arquivo de som inválido.".into()));
    }
    match fs::remove_file(candidate) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Storage(error.to_string())),
    }
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, AppError> {
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(path).map_err(|e| AppError::Storage(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| AppError::Storage(e.to_string()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::Storage(e.to_string()))?;
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), stamp));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(value).map_err(|e| AppError::Storage(e.to_string()))?,
    )
    .map_err(|e| AppError::Storage(e.to_string()))?;
    fs::rename(tmp, path).map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(())
}

pub fn get_settings(app: &tauri::AppHandle) -> Result<Settings, AppError> {
    read_json(&data_dir(app)?.join("settings.json"))
}

pub fn save_settings(app: &tauri::AppHandle, settings: &Settings) -> Result<(), AppError> {
    write_json(&data_dir(app)?.join("settings.json"), settings)
}

pub fn get_window_state(app: &tauri::AppHandle) -> Result<WindowState, AppError> {
    read_json(&data_dir(app)?.join("window.json"))
}

pub fn save_window_state(app: &tauri::AppHandle, state: &WindowState) -> Result<(), AppError> {
    write_json(&data_dir(app)?.join("window.json"), state)
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cache {
    pub calendars: Vec<crate::models::CalendarInfo>,
    pub events: Vec<CalendarEvent>,
    pub sync_tokens: HashMap<String, String>,
}

/// Single lock for every cache read-modify-write. Never held across an `.await`.
static CACHE_LOCK: Mutex<()> = Mutex::new(());
static CACHE_MEMORY: Mutex<Option<Cache>> = Mutex::new(None);

/// Loads the cache from disk. Callers must hold CACHE_LOCK.
fn load_cache(app: &tauri::AppHandle) -> Result<Cache, AppError> {
    let path = data_dir(app)?.join("cache.json");
    if !path.exists() {
        return Ok(Cache::default());
    }
    // An I/O error is transient: surface it, never wipe the cache because of it.
    let text = fs::read_to_string(&path).map_err(|e| AppError::Storage(e.to_string()))?;
    match serde_json::from_str::<Cache>(&text) {
        Ok(cache) => Ok(cache),
        Err(error) => {
            // Writes are atomic now, so a parse failure means real corruption.
            // Keep a copy for inspection instead of destroying the evidence.
            let _ = fs::copy(&path, path.with_extension("corrupt.json"));
            log::warn!("Local calendar cache is corrupt and was reset: {error}");
            Ok(Cache::default())
        }
    }
}

/// Runs `f` on the cache under the lock and persists the result (read-modify-write, atomic w.r.t. other callers).
fn with_cache<R>(app: &tauri::AppHandle, f: impl FnOnce(&mut Cache) -> R) -> Result<R, AppError> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory = CACHE_MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    if memory.is_none() {
        *memory = Some(load_cache(app)?);
    }
    let cache = memory.as_mut().unwrap();
    let result = f(cache);
    write_json(&data_dir(app)?.join("cache.json"), cache)?;
    log::debug!("cache saved: {} events, {} calendars", cache.events.len(), cache.calendars.len());
    Ok(result)
}

fn read_cache<R>(app: &tauri::AppHandle, f: impl FnOnce(&Cache) -> R) -> Result<R, AppError> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory = CACHE_MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    if memory.is_none() {
        *memory = Some(load_cache(app)?);
    }
    Ok(f(memory.as_ref().unwrap()))
}

pub fn save_calendars(app: &tauri::AppHandle, calendars: Vec<crate::models::CalendarInfo>) -> Result<(), AppError> {
    with_cache(app, |cache| {
        if cache.calendars != calendars {
            cache.calendars = calendars;
        }
    })
}

pub fn cached_calendars(app: &tauri::AppHandle) -> Result<Vec<crate::models::CalendarInfo>, AppError> {
    read_cache(app, |cache| cache.calendars.clone())
}

pub fn clear_cache(app: &tauri::AppHandle) -> Result<(), AppError> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory = CACHE_MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    *memory = None;
    let path = data_dir(app)?.join("cache.json");
    match fs::remove_file(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Storage(error.to_string())),
    }
}

pub fn merge_events(app: &tauri::AppHandle, incoming: Vec<CalendarEvent>) -> Result<(), AppError> {
    with_cache(app, |cache| {
        for event in incoming {
            if let Some(existing) = cache.events.iter_mut().find(|x| x.id == event.id && x.calendar_id == event.calendar_id) {
                *existing = event;
            } else {
                cache.events.push(event);
            }
        }
    })
}

/// Full (non-incremental) sync result: swaps one calendar's events and token in a single write,
/// so readers never observe an emptied calendar.
pub fn replace_calendar_events(
    app: &tauri::AppHandle,
    calendar_id: &str,
    events: Vec<CalendarEvent>,
    sync_token: Option<String>,
) -> Result<(), AppError> {
    with_cache(app, |cache| {
        cache.events.retain(|e| e.calendar_id != calendar_id);
        cache.events.extend(events);
        match sync_token {
            Some(token) => { cache.sync_tokens.insert(calendar_id.to_string(), token); }
            None => { cache.sync_tokens.remove(calendar_id); }
        }
    })
}

fn apply_event_changes(
    cache: &mut Cache,
    calendar_id: &str,
    changed: Vec<CalendarEvent>,
    removed_event_ids: &[String],
    sync_token: String,
) {
    let removed: std::collections::HashSet<&str> = removed_event_ids.iter().map(String::as_str).collect();
    cache.events.retain(|event| {
        !(event.calendar_id == calendar_id && removed.contains(event.id.as_str()))
    });
    for event in changed {
        if event.status.as_deref() == Some("cancelled") {
            continue;
        }
        if let Some(existing) = cache.events.iter_mut().find(|x| x.id == event.id && x.calendar_id == event.calendar_id) {
            *existing = event;
        } else {
            cache.events.push(event);
        }
    }
    cache.sync_tokens.insert(calendar_id.to_string(), sync_token);
}

#[derive(Debug)]
pub struct SyncCalendarUpdate {
    pub calendar_id: String,
    pub full: bool,
    pub changed: Vec<CalendarEvent>,
    pub removed_event_ids: Vec<String>,
    pub sync_token: Option<String>,
}

pub fn apply_sync_batch(
    app: &tauri::AppHandle,
    calendars: Vec<crate::models::CalendarInfo>,
    updates: Vec<SyncCalendarUpdate>,
) -> Result<usize, AppError> {
    let changed_count = updates.iter().map(|update| update.changed.len()).sum();
    let _guard = CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut memory = CACHE_MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    if memory.is_none() {
        *memory = Some(load_cache(app)?);
    }
    let cache = memory.as_mut().unwrap();
    let mut dirty = cache.calendars != calendars;
    cache.calendars = calendars;

    for update in updates {
        if update.full {
            cache.events.retain(|event| event.calendar_id != update.calendar_id);
            cache.events.extend(update.changed);
            dirty = true;
            match update.sync_token {
                Some(token) => {
                    if cache.sync_tokens.get(&update.calendar_id) != Some(&token) {
                        cache.sync_tokens.insert(update.calendar_id, token);
                    }
                }
                None => { dirty = cache.sync_tokens.remove(&update.calendar_id).is_some() || dirty; }
            }
        } else if let Some(token) = update.sync_token {
            let old_token = cache.sync_tokens.get(&update.calendar_id).cloned();
            let had_changed_events = !update.changed.is_empty();
            apply_event_changes(
                cache,
                &update.calendar_id,
                update.changed,
                &update.removed_event_ids,
                token.clone(),
            );
            dirty = dirty
                || !update.removed_event_ids.is_empty()
                || cache.sync_tokens.get(&update.calendar_id) != old_token.as_ref()
                || had_changed_events;
        } else {
            log::warn!(
                "incremental sync for calendar={} returned no next sync token; changes were not persisted",
                update.calendar_id
            );
        }
    }

    if dirty {
        write_json(&data_dir(app)?.join("cache.json"), cache)?;
        log::debug!(
            "cache saved: {} events, {} calendars",
            cache.events.len(),
            cache.calendars.len()
        );
    } else {
        log::debug!("cache unchanged; skipped disk write");
    }
    Ok(changed_count)
}

pub fn apply_incremental_changes(
    app: &tauri::AppHandle,
    calendar_id: &str,
    changed: Vec<CalendarEvent>,
    removed_event_ids: &[String],
    sync_token: String,
) -> Result<(), AppError> {
    with_cache(app, |cache| apply_event_changes(cache, calendar_id, changed, removed_event_ids, sync_token))
}

pub fn remove_event(app: &tauri::AppHandle, calendar_id: &str, event_id: &str) -> Result<(), AppError> {
    with_cache(app, |cache| cache.events.retain(|e| !(e.calendar_id == calendar_id && e.id == event_id)))
}

pub fn cached_events(app: &tauri::AppHandle, start: &str, end: &str) -> Result<Vec<CalendarEvent>, AppError> {
    let events: Vec<CalendarEvent> = read_cache(app, |cache| {
        cache.events.iter().filter(|e| overlaps(e, start, end)).cloned().collect()
    })?;
    log::debug!("get_cached_events -> {} events", events.len());
    Ok(events)
}

fn overlaps(event: &CalendarEvent, start: &str, end: &str) -> bool {
    let event_start = event.start.date_time.as_deref().or(event.start.date.as_deref()).unwrap_or("");
    let event_end = event.end.date_time.as_deref().or(event.end.date.as_deref()).unwrap_or("");

    if let (Some(query_start), Some(query_end), Some(event_start_dt), Some(event_end_dt)) = (
        parse_rfc3339(start),
        parse_rfc3339(end),
        parse_rfc3339(event_start),
        parse_rfc3339(event_end),
    ) {
        return event_start_dt < query_end && event_end_dt > query_start;
    }

    // Date-only Google Calendar values use YYYY-MM-DD and sort correctly
    // lexicographically. Keep this fallback for all-day events.
    event_start < end && event_end > start
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|value| value.with_timezone(&Utc))
}

pub fn get_sync_token(app: &tauri::AppHandle, calendar_id: &str) -> Result<Option<String>, AppError> {
    read_cache(app, |cache| cache.sync_tokens.get(calendar_id).cloned())
}

pub fn save_refresh_token(token: &str) -> Result<(), AppError> {
    let entry = Entry::new(SERVICE, REFRESH_TOKEN).map_err(|e| AppError::Storage(e.to_string()))?;
    entry.set_password(token).map_err(|e| AppError::Storage(e.to_string()))
}

pub fn get_refresh_token() -> Result<Option<String>, AppError> {
    let entry = Entry::new(SERVICE, REFRESH_TOKEN).map_err(|e| AppError::Storage(e.to_string()))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(AppError::Storage(error.to_string())),
    }
}

pub fn delete_refresh_token() -> Result<(), AppError> {
    let entry = Entry::new(SERVICE, REFRESH_TOKEN).map_err(|e| AppError::Storage(e.to_string()))?;
    match entry.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::Storage(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EventTime;

    fn event(id: &str, calendar_id: &str, summary: &str) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            calendar_id: calendar_id.into(),
            summary: summary.into(),
            start: EventTime { date: Some("2026-09-10".into()), ..Default::default() },
            end: EventTime { date: Some("2026-09-11".into()), ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn incremental_cache_replaces_adds_removes_and_advances_token() {
        let mut cache = Cache { events: vec![event("old", "c", "Old"), event("keep", "c", "Keep")], sync_tokens: HashMap::new() };
        let changed = vec![event("keep", "c", "Updated"), event("new", "c", "New")];
        let removed = vec!["old".to_string()];

        apply_event_changes(&mut cache, "c", &changed, &removed, "token-2".into());

        assert_eq!(cache.events.len(), 2);
        assert!(cache.events.iter().any(|e| e.id == "keep" && e.summary == "Updated"));
        assert!(cache.events.iter().any(|e| e.id == "new"));
        assert!(!cache.events.iter().any(|e| e.id == "old"));
        assert_eq!(cache.sync_tokens.get("c"), Some(&"token-2".to_string()));
    }



}
