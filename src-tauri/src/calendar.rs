use crate::{
    auth::AuthManager,
    error::AppError,
    models::{CalendarEvent, CalendarInfo, EventTime},
    storage,
};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const BASE: &str = "https://www.googleapis.com/calendar/v3";

#[derive(Debug, Deserialize)]
struct CalendarListResponse {
    items: Option<Vec<CalendarInfo>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    items: Option<Vec<GoogleEvent>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "nextSyncToken")]
    next_sync_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct GoogleEvent {
    id: String,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    status: Option<String>,
    #[serde(rename = "colorId")]
    color_id: Option<String>,
    #[serde(rename = "htmlLink")]
    html_link: Option<String>,
    start: EventTime,
    end: EventTime,
    #[serde(rename = "recurringEventId")]
    recurring_event_id: Option<String>,
    #[serde(rename = "originalStartTime")]
    original_start_time: Option<EventTime>,
}

impl GoogleEvent {
    fn into_model(
        self,
        calendar_id: &str,
        calendar_color: Option<String>,
        event_color: Option<String>,
    ) -> CalendarEvent {
        CalendarEvent {
            id: self.id,
            calendar_id: calendar_id.to_string(),
            summary: self.summary.unwrap_or_default(),
            description: self.description,
            location: self.location,
            status: self.status,
            color_id: self.color_id,
            event_color,
            calendar_color,
            html_link: self.html_link,
            start: self.start,
            end: self.end,
            recurring_event_id: self.recurring_event_id,
            original_start_time: self.original_start_time,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ColorsResponse {
    event: Option<HashMap<String, GoogleColor>>,
}

#[derive(Debug, Deserialize)]
struct GoogleColor {
    background: Option<String>,
}

pub async fn list_calendars(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
) -> Result<Vec<CalendarInfo>, AppError> {
    let token = auth.access_token(app).await?;
    let client = crate::http::client();
    let mut page_token: Option<String> = None;
    let mut calendars = Vec::new();

    loop {
        let mut query = vec![("maxResults", "250")];
        if let Some(page) = page_token.as_deref() {
            query.push(("pageToken", page));
        }

        let response = client
            .get(format!("{BASE}/users/me/calendarList"))
            .bearer_auth(&token)
            .query(&query)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(AppError::AuthRequired);
        }

        let body = response
            .error_for_status()
            .map_err(|e| AppError::Google(e.to_string()))?
            .json::<CalendarListResponse>()
            .await
            .map_err(|e| AppError::Google(e.to_string()))?;

        calendars.extend(body.items.unwrap_or_default());
        match body.next_page_token {
            Some(next) => page_token = Some(next),
            None => break,
        }
    }

    Ok(calendars)
}

pub async fn list_events(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
    start: &str,
    end: &str,
    calendar_ids: &[String],
) -> Result<Vec<CalendarEvent>, AppError> {
    let calendars = list_calendars(auth, app).await?;
    let colors = fetch_event_colors(auth, app).await.unwrap_or_default();
    let mut output = Vec::new();
    let client = crate::http::client();

    for calendar_id in calendar_ids {
        let calendar_color = calendars
            .iter()
            .find(|calendar| &calendar.id == calendar_id)
            .and_then(|calendar| calendar.background_color.clone());

        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![
                ("timeMin", start.to_string()),
                ("timeMax", end.to_string()),
                ("singleEvents", "true".to_string()),
                ("showDeleted", "false".to_string()),
                ("maxResults", "2500".to_string()),
            ];
            if let Some(page) = page_token.as_ref() {
                query.push(("pageToken", page.clone()));
            }

            let response = client
                .get(format!(
                    "{BASE}/calendars/{}/events",
                    urlencoding::encode(calendar_id)
                ))
                .bearer_auth(auth.access_token(app).await?)
                .query(&query)
                .send()
                .await
                .map_err(|e| AppError::Network(e.to_string()))?;

            if response.status() == StatusCode::UNAUTHORIZED {
                return Err(AppError::AuthRequired);
            }
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::Google(
                    "O limite de requisições do Google Agenda foi atingido. Tente novamente mais tarde.".into(),
                ));
            }

            let body = response
                .error_for_status()
                .map_err(|e| AppError::Google(e.to_string()))?
                .json::<EventsResponse>()
                .await
                .map_err(|e| AppError::Google(e.to_string()))?;

            output.extend(body.items.unwrap_or_default().into_iter().map(|event| {
                let event_color = event
                    .color_id
                    .as_ref()
                    .and_then(|id| colors.get(id).cloned());
                event.into_model(calendar_id, calendar_color.clone(), event_color)
            }));

            if body.next_page_token.is_none() {
                break;
            }
            page_token = body.next_page_token;
        }
    }

    Ok(output)
}

pub async fn create_event(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
    event: &CalendarEvent,
) -> Result<CalendarEvent, AppError> {
    validate_event(event)?;
    let token = auth.access_token(app).await?;
    let body = to_google_body(event)?;
    let response = crate::http::client()
        .post(format!(
            "{BASE}/calendars/{}/events",
            urlencoding::encode(&event.calendar_id)
        ))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthRequired);
    }

    let google = response
        .error_for_status()
        .map_err(|e| AppError::Google(e.to_string()))?
        .json::<GoogleEvent>()
        .await
        .map_err(|e| AppError::Google(e.to_string()))?;

    Ok(google.into_model(
        &event.calendar_id,
        event.calendar_color.clone(),
        event.event_color.clone(),
    ))
}

pub async fn update_event(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
    event: &CalendarEvent,
) -> Result<CalendarEvent, AppError> {
    validate_event(event)?;
    let token = auth.access_token(app).await?;
    let body = to_google_body(event)?;
    let response = crate::http::client()
        .patch(format!(
            "{BASE}/calendars/{}/events/{}",
            urlencoding::encode(&event.calendar_id),
            urlencoding::encode(&event.id)
        ))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthRequired);
    }

    let google = response
        .error_for_status()
        .map_err(|e| AppError::Google(e.to_string()))?
        .json::<GoogleEvent>()
        .await
        .map_err(|e| AppError::Google(e.to_string()))?;

    Ok(google.into_model(
        &event.calendar_id,
        event.calendar_color.clone(),
        event.event_color.clone(),
    ))
}

pub async fn delete_event(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
    calendar_id: &str,
    event_id: &str,
) -> Result<(), AppError> {
    let token = auth.access_token(app).await?;
    let response = crate::http::client()
        .delete(format!(
            "{BASE}/calendars/{}/events/{}",
            urlencoding::encode(calendar_id),
            urlencoding::encode(event_id)
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthRequired);
    }
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(());
    }
    response
        .error_for_status()
        .map_err(|e| AppError::Google(e.to_string()))?;
    Ok(())
}

fn validate_event(event: &CalendarEvent) -> Result<(), AppError> {
    if event.calendar_id.trim().is_empty() {
        return Err(AppError::InvalidData("O calendário é obrigatório.".into()));
    }
    if event.summary.trim().is_empty() {
        return Err(AppError::InvalidData("O título do evento é obrigatório.".into()));
    }
    let has_start = event.start.date.is_some() || event.start.date_time.is_some();
    let has_end = event.end.date.is_some() || event.end.date_time.is_some();
    if !has_start || !has_end {
        return Err(AppError::InvalidData("Início e fim do evento são obrigatórios.".into()));
    }
    Ok(())
}

fn to_google_body(event: &CalendarEvent) -> Result<Value, AppError> {
    let mut value = json!({
        "summary": event.summary.trim(),
        "description": event.description.as_deref().filter(|s| !s.is_empty()),
        "location": event.location.as_deref().filter(|s| !s.is_empty()),
    });

    if let Some(date) = &event.start.date {
        value["start"] = json!({ "date": date });
    } else if let Some(date_time) = &event.start.date_time {
        value["start"] = json!({
            "dateTime": date_time,
            "timeZone": event.start.time_zone,
        });
    }

    if let Some(date) = &event.end.date {
        value["end"] = json!({ "date": date });
    } else if let Some(date_time) = &event.end.date_time {
        value["end"] = json!({
            "dateTime": date_time,
            "timeZone": event.end.time_zone,
        });
    }

    Ok(value)
}

async fn fetch_event_colors(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
) -> Result<HashMap<String, String>, AppError> {
    let token = auth.access_token(app).await?;
    let response = crate::http::client()
        .get(format!("{BASE}/colors"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(AppError::AuthRequired);
    }
    let body = response
        .error_for_status()
        .map_err(|e| AppError::Google(e.to_string()))?
        .json::<ColorsResponse>()
        .await
        .map_err(|e| AppError::Google(e.to_string()))?;
    Ok(body
        .event
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(id, color)| color.background.map(|value| (id, value)))
        .collect())
}

async fn fetch_page(
    auth: &mut AuthManager,
    app: &tauri::AppHandle,
    calendar_id: &str,
    calendar_color: Option<String>,
    event_colors: &HashMap<String, String>,
    start: Option<&str>,
    end: Option<&str>,
    sync_token: Option<&str>,
) -> Result<(Vec<CalendarEvent>, Vec<String>, Option<String>), AppError> {
    let client = crate::http::client();
    let mut next_page: Option<String> = None;
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut next_sync = None;

    loop {
        let mut query = vec![
            ("singleEvents", "true".to_string()),
            ("showDeleted", "true".to_string()),
            ("maxResults", "2500".to_string()),
        ];
        if let Some(start) = start {
            query.push(("timeMin", start.to_string()));
        }
        if let Some(end) = end {
            query.push(("timeMax", end.to_string()));
        }
        if let Some(token) = sync_token {
            query.push(("syncToken", token.to_string()));
        }
        if let Some(page) = &next_page {
            query.push(("pageToken", page.clone()));
        }

        let response = client
            .get(format!(
                "{BASE}/calendars/{}/events",
                urlencoding::encode(calendar_id)
            ))
            .bearer_auth(auth.access_token(app).await?)
            .query(&query)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(AppError::AuthRequired);
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Google(
                "O limite de requisições do Google Agenda foi atingido. Tente novamente mais tarde.".into(),
            ));
        }
        if response.status() == StatusCode::GONE {
            return Err(AppError::SyncTokenExpired);
        }

        let body = response
            .error_for_status()
            .map_err(|e| AppError::Google(e.to_string()))?
            .json::<EventsResponse>()
            .await
            .map_err(|e| AppError::Google(e.to_string()))?;

        for event in body.items.unwrap_or_default() {
            let id = event.id.clone();
            if event.status.as_deref() == Some("cancelled") {
                removed.push(id);
                continue;
            }
            let color = event
                .color_id
                .as_ref()
                .and_then(|color_id| event_colors.get(color_id).cloned());
            changed.push(event.into_model(calendar_id, calendar_color.clone(), color));
        }

        next_sync = body.next_sync_token.or(next_sync);
        match body.next_page_token {
            Some(page) => next_page = Some(page),
            None => break,
        }
    }

    Ok((changed, removed, next_sync))
}

pub async fn sync(
auth: &mut AuthManager,
app: &tauri::AppHandle,
) -> Result<usize, AppError> {
    let settings = storage::get_settings(app)?;
    let calendar_ids = settings.visible_calendar_ids;
    if calendar_ids.is_empty() {
        return Ok(0);
    }

    let calendars = list_calendars(auth, app).await?;
    let event_colors = fetch_event_colors(auth, app).await.unwrap_or_default();
    let mut updates = Vec::with_capacity(calendar_ids.len());

    for calendar_id in calendar_ids {
        let calendar_color = calendars
            .iter()
            .find(|calendar| calendar.id == calendar_id)
            .and_then(|calendar| calendar.background_color.clone());

        // Google requires incremental requests to preserve the same query
        // shape as the original request; timeMin/timeMax cannot accompany
        // syncToken. Use an unbounded initial full sync so the token remains
        // valid for subsequent incremental synchronization.
        let token = storage::get_sync_token(app, &calendar_id)?;
        log::debug!(
            "sync: calendar={} token_present={}",
            calendar_id,
            token.is_some()
        );
        let mut full = token.is_none();
        let result = fetch_page(
            auth,
            app,
            &calendar_id,
            calendar_color.clone(),
            &event_colors,
            None,
            None,
            token.as_deref(),
        )
        .await;

        let (changed, removed, next_sync) = match result {
            Ok(value) => value,
            Err(AppError::SyncTokenExpired) => {
                // Token invalid: refetch everything. All successful calendar
                // updates are committed together after the network phase.
                full = true;
                fetch_page(
                    auth,
                    app,
                    &calendar_id,
                    calendar_color,
                    &event_colors,
                    None,
                    None,
                    None,
                )
                .await?
            }
            Err(error) => return Err(error),
        };

        log::debug!(
            "sync: calendar={} mode={} changed={} removed={} next_token_present={}",
            calendar_id,
            if full { "full" } else { "incremental" },
            changed.len(),
            removed.len(),
            next_sync.is_some()
        );
        updates.push(storage::SyncCalendarUpdate {
            calendar_id,
            full,
            changed,
            removed_event_ids: removed,
            sync_token: next_sync,
        });
    }

    // One atomic cache write for the whole sync instead of one write per
    // calendar, while keeping all network awaits outside storage locks.
    storage::apply_sync_batch(app, calendars, updates)
}

