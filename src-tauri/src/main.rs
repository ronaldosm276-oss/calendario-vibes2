#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod auth;
mod calendar;
mod error;
mod http;
mod models;
mod storage;
mod sync;
mod reminders;

use auth::AuthManager;
use error::AppError;
use models::*;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, State, WindowEvent};

struct AppState {
    auth: tokio::sync::Mutex<AuthManager>,
}


static ALLOW_EXIT: AtomicBool = AtomicBool::new(false);

#[tauri::command]
async fn get_auth_state(state: State<'_, AppState>) -> Result<AuthState, AppError> {
    let mut auth = state.auth.lock().await;
    if !auth.state.authenticated && storage::get_refresh_token()?.is_some() {
        auth.state.authenticated = true;
    }
    Ok(auth.state.clone())
}

#[tauri::command]
async fn sign_in(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AuthState, AppError> {
    let mut auth = state.auth.lock().await;
    auth.sign_in(&app).await
}

#[tauri::command]
async fn sign_out(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    state.auth.lock().await.sign_out()?;
    storage::clear_cache(&app)
}

#[tauri::command]
async fn list_calendars(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<CalendarInfo>, AppError> {
    let mut auth = state.auth.lock().await;
    let calendars = calendar::list_calendars(&mut auth, &app).await?;
    storage::save_calendars(&app, calendars.clone())?;
    Ok(calendars)
}

#[tauri::command]
async fn list_events(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    start: String,
    end: String,
    calendar_ids: Vec<String>,
) -> Result<Vec<CalendarEvent>, AppError> {
    let mut auth = state.auth.lock().await;
    calendar::list_events(&mut auth, &app, &start, &end, &calendar_ids).await
}

#[tauri::command]
async fn create_event(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    event: CalendarEvent,
) -> Result<CalendarEvent, AppError> {
    let mut auth = state.auth.lock().await;
    let saved = calendar::create_event(&mut auth, &app, &event).await?;
    storage::merge_events(&app, vec![saved.clone()])?;
    Ok(saved)
}

#[tauri::command]
async fn update_event(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    event: CalendarEvent,
) -> Result<CalendarEvent, AppError> {
    if event.id.is_empty() {
        return Err(AppError::InvalidData("O ID do evento é obrigatório para editar.".into()));
    }
    // Google Calendar's event id is scoped to its calendar. Moving an existing
    // event requires a separate move operation; the MVP intentionally edits
    // the event in-place, so prevent an accidental PATCH against another
    // calendar id.
    // The frontend keeps the original calendar selected while editing.
    let mut auth = state.auth.lock().await;
    let saved = calendar::update_event(&mut auth, &app, &event).await?;
    storage::merge_events(&app, vec![saved.clone()])?;
    Ok(saved)
}

#[tauri::command]
async fn delete_event(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    calendar_id: String,
    event_id: String,
) -> Result<(), AppError> {
    let mut auth = state.auth.lock().await;
    calendar::delete_event(&mut auth, &app, &calendar_id, &event_id).await?;
    storage::remove_event(&app, &calendar_id, &event_id)
}

#[tauri::command]
async fn sync_now(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncState, AppError> {
    let mut auth = state.auth.lock().await;
    sync::sync_now(&mut auth, &app).await
}

#[tauri::command]
async fn get_cached_calendars(app: tauri::AppHandle) -> Result<Vec<CalendarInfo>, AppError> {
    storage::cached_calendars(&app)
}

#[tauri::command]
async fn get_cached_events(
    app: tauri::AppHandle,
    start: String,
    end: String,
) -> Result<Vec<CalendarEvent>, AppError> {
    storage::cached_events(&app, &start, &end)
}

#[tauri::command]
async fn get_settings(app: tauri::AppHandle) -> Result<Settings, AppError> {
    storage::get_settings(&app)
}

#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    settings: Settings,
) -> Result<(), AppError> {
    storage::save_settings(&app, &settings)?;
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(settings.always_on_top)
            .map_err(|e| AppError::Storage(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
async fn save_window_state(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
    width: f64,
    height: f64,
) -> Result<(), AppError> {
    storage::save_window_state(
        &app,
        &WindowState {
            x: Some(x),
            y: Some(y),
            width: Some(width),
            height: Some(height),
        },
    )
}

#[tauri::command]
async fn get_window_state(app: tauri::AppHandle) -> Result<WindowState, AppError> {
    storage::get_window_state(&app)
}

fn save_current_window_state(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return; };
    let Ok(position) = window.outer_position() else { return; };
    let Ok(size) = window.inner_size() else { return; };
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let logical_size = size.to_logical::<f64>(scale_factor);
    let _ = storage::save_window_state(
        app,
        &WindowState {
            x: Some(position.x),
            y: Some(position.y),
            width: Some(logical_size.width),
            height: Some(logical_size.height),
        },
    );
}

#[tauri::command]
fn list_sounds(app: tauri::AppHandle) -> Result<Vec<SoundInfo>, AppError> {
    storage::list_sounds(&app)
}

#[tauri::command]
fn add_sound(app: tauri::AppHandle, source_path: String) -> Result<SoundInfo, AppError> {
    storage::add_sound(&app, Path::new(&source_path))
}

#[tauri::command]
fn remove_sound(app: tauri::AppHandle, name: String) -> Result<(), AppError> {
    storage::remove_sound(&app, &name)
}

#[tauri::command]
fn get_sounds_dir(app: tauri::AppHandle) -> Result<String, AppError> {
    Ok(storage::sounds_dir(&app)?.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_sounds_folder(app: tauri::AppHandle) -> Result<(), AppError> {
    let dir = storage::sounds_dir(&app)?;
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(())
}

#[tauri::command]
fn play_sound(app: tauri::AppHandle, audio: tauri::State<'_, audio::AudioController>, sound: Option<String>, volume: f32, looped: bool) -> Result<(), AppError> {
    let path = reminders::resolve_sound_path(&app, sound.as_deref())?;
    audio.play(path, volume, looped)
}

#[tauri::command]
fn stop_sound(audio: tauri::State<'_, audio::AudioController>) -> Result<(), AppError> {
    audio.stop()
}

#[tauri::command]
fn get_active_alarms(service: tauri::State<'_, reminders::ReminderService>) -> Vec<AlarmInfo> {
    service.active()
}

#[tauri::command]
fn acknowledge_alarm(service: tauri::State<'_, reminders::ReminderService>, id: String) -> Result<(), AppError> {
    service.acknowledge(&id)
}

#[tauri::command]
fn snooze_alarm(service: tauri::State<'_, reminders::ReminderService>, id: String, minutes: u32) -> Result<(), AppError> {
    service.snooze(&id, minutes)
}

fn present_main_window(window: &tauri::WebviewWindow) {
    let minimized = window.is_minimized().unwrap_or(false);
    let visible = window.is_visible().unwrap_or(false);

    if minimized {
        let _ = window.unminimize();
    }
    if !visible {
        let _ = window.show();
    }

    // A remembered position can become invalid after monitor/desk changes.
    // In that case, center the window instead of leaving the app effectively invisible.
    if window.current_monitor().ok().flatten().is_none() {
        let _ = window.center();
    }
    let _ = window.set_focus();
}

fn setup_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Mostrar calendário", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Ocultar calendário", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    TrayIconBuilder::new()
        .icon(tauri::include_image!("icons/32x32.png"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Calendário")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    present_main_window(&window);
                }
            }
            "hide" => {
                save_current_window_state(app);
                if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); }
            }
            "quit" => {
                save_current_window_state(app);
                ALLOW_EXIT.store(true, Ordering::Relaxed);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    let minimized = window.is_minimized().unwrap_or(false);
                    let has_monitor = window.current_monitor().ok().flatten().is_some();

                    if visible && !minimized && has_monitor {
                        save_current_window_state(tray.app_handle());
                        let _ = window.hide();
                    } else {
                        present_main_window(&window);
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState { auth: tokio::sync::Mutex::new(AuthManager::default()) };
    let audio_controller = audio::AudioController::start();
    let reminder_service = reminders::ReminderService::new(audio_controller.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                present_main_window(&window);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(state)
        .manage(audio_controller)
        .manage(reminder_service.clone())
        .setup(move |app| {
            setup_tray(app.handle()).map_err(|e| Box::<dyn std::error::Error>::from(e))?;
            reminders::start_scheduler(app.handle().clone(), reminder_service.clone());

            if let Some(alarm_window) = app.get_webview_window("alarm") {
                let alarm_service = reminder_service.clone();
                alarm_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if !alarm_service.active().is_empty() { api.prevent_close(); }
                    }
                });
                let _ = alarm_window.hide();
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let mut auth = state.auth.lock().await;
                if let Err(error) = auth.restore(&app_handle).await {
                    log::warn!("Could not restore Google authentication: {error}");
                }
            });

            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { .. } = event {
                        save_current_window_state(&app_handle);
                        ALLOW_EXIT.store(true, Ordering::Relaxed);
                    }
                });

                let settings = storage::get_settings(app.handle()).unwrap_or_default();
                let window_state = storage::get_window_state(app.handle()).unwrap_or_default();
                if settings.remember_position {
                    if let (Some(x), Some(y)) = (window_state.x, window_state.y) {
                        let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
                    }
                }
                if settings.remember_size {
                    if let (Some(width), Some(height)) = (window_state.width, window_state.height) {
                        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
                    }
                }
                let _ = window.set_always_on_top(settings.always_on_top);
                // Development builds must remain visible so a persisted
                // "start hidden" preference cannot make `tauri dev` appear frozen/invisible.
                // The preference is still honored in release builds.
                if settings.start_hidden && !cfg!(debug_assertions) {
                    let _ = window.hide();
                } else {
                    present_main_window(&window);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_auth_state,
            sign_in,
            sign_out,
            list_calendars,
            list_events,
            create_event,
            update_event,
            delete_event,
            sync_now,
            get_cached_calendars,
            get_cached_events,
            get_settings,
            save_settings,
            save_window_state,
            get_window_state,
            list_sounds,
            add_sound,
            remove_sound,
            get_sounds_dir,
            open_sounds_folder,
            play_sound,
            stop_sound,
            get_active_alarms,
            acknowledge_alarm,
            snooze_alarm
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                save_current_window_state(app);
                if !ALLOW_EXIT.load(Ordering::Relaxed) { api.prevent_exit(); }
            }
        });
}

fn main() {
    env_logger::init();
    run();
}
