use crate::{auth::AuthManager, calendar, error::AppError, models::SyncState};
use chrono::Utc;

pub async fn sync_now(auth:&mut AuthManager,app:&tauri::AppHandle)->Result<SyncState,AppError>{let started=std::time::Instant::now();log::debug!("sync_now: start");let count=calendar::sync(auth,app).await?;log::debug!("sync_now: done in {:?}, {} changed",started.elapsed(),count);Ok(SyncState{status:"synced".into(),last_synced_at:Some(Utc::now().to_rfc3339()),message:Some(format!("{count} events synchronized"))})}
