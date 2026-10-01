use crate::{error::AppError, models::AuthState, storage};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, time::{Duration, Instant}};
use tauri::Manager;
use tiny_http::{Header, Response, Server};
use url::Url;

const SCOPES: &str = "https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone, Deserialize)]
struct ClientJson { installed: Option<ClientInstalled>, web: Option<ClientInstalled> }
#[derive(Debug, Clone, Deserialize)]
struct ClientInstalled { client_id: String, client_secret: Option<String>, auth_uri: Option<String>, token_uri: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse { access_token: String, expires_in: u64, refresh_token: Option<String>, token_type: String, scope: Option<String> }
#[derive(Debug, Clone)]
pub struct AuthManager { pub state: AuthState, access_token: Option<String>, expires_at: Option<Instant> }
impl Default for AuthManager { fn default()->Self{Self{state:AuthState::default(),access_token:None,expires_at:None}} }

fn client_config(app:&tauri::AppHandle)->Result<ClientInstalled,AppError>{
  let resource_path = app.path().resolve("google-client.json", tauri::path::BaseDirectory::Resource).ok();
  let dev_path = std::env::current_dir().ok().map(|dir| dir.join("src-tauri").join("google-client.json"));
  let candidates = resource_path.into_iter().chain(dev_path);
  let mut last_path = None;
  for path in candidates {
    last_path = Some(path.clone());
    if let Ok(text) = fs::read_to_string(&path) {
      let cfg:ClientJson=serde_json::from_str(&text).map_err(|e|AppError::Config(e.to_string()))?;
      return cfg.installed.or(cfg.web).ok_or_else(||AppError::Config("google-client.json must contain an installed OAuth client".into()));
    }
  }
  let path = last_path.unwrap_or_else(|| std::path::PathBuf::from("src-tauri/google-client.json"));
  Err(AppError::Config(format!("Missing Google OAuth client file: {}",path.display())))
}

fn random_string(n:usize)->String{rand::rng().sample_iter(&Alphanumeric).take(n).map(char::from).collect()}

impl AuthManager {
 pub async fn restore(&mut self, app:&tauri::AppHandle)->Result<(),AppError>{
  if storage::get_refresh_token()?.is_some(){
   self.state.authenticated=true;
   if let Err(error)=self.refresh_access_token(app).await{
    if matches!(error, AppError::Network(_)) {
     log::warn!("Google authentication refresh unavailable; continuing with cached calendar data");
     return Ok(());
    }
    self.state.authenticated=false;
    return Err(error);
   }
  }
  Ok(())
 }
 pub async fn sign_in(&mut self, app:&tauri::AppHandle)->Result<AuthState,AppError>{
  let cfg=client_config(app)?; let verifier=random_string(96); let challenge=URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())); let state=random_string(48);
  let server=Server::http("127.0.0.1:0").map_err(|e|AppError::OAuth(e.to_string()))?; let addr=server.server_addr(); let port=addr.to_ip().ok_or_else(||AppError::OAuth("Invalid loopback address".into()))?.port();
  let redirect=format!("http://127.0.0.1:{port}"); let mut url=Url::parse(&cfg.auth_uri.unwrap_or_else(||AUTH_URL.into())).map_err(|e|AppError::OAuth(e.to_string()))?;
  url.query_pairs_mut().append_pair("client_id",&cfg.client_id).append_pair("redirect_uri",&redirect).append_pair("response_type","code").append_pair("scope",SCOPES).append_pair("access_type","offline").append_pair("prompt","consent").append_pair("code_challenge",&challenge).append_pair("code_challenge_method","S256").append_pair("state",&state);
  tauri_plugin_opener::open_url(url.as_str(),None::<&str>).map_err(|e|AppError::OAuth(e.to_string()))?;
  let request=tokio::task::spawn_blocking(move || server.recv_timeout(Duration::from_secs(300))).await.map_err(|e|AppError::OAuth(e.to_string()))?.map_err(|e|AppError::OAuth(e.to_string()))?.ok_or_else(||AppError::OAuth("OAuth timed out".into()))?;
  let query=request.url().split('?').nth(1).unwrap_or(""); let params:std::collections::HashMap<_,_>=url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
  let ok_html="<html><body><h3>Calendar Gadget</h3><p>Authorization complete. You can close this tab.</p></body></html>"; let _=request.respond(Response::from_string(ok_html).with_header(Header::from_bytes("Content-Type", "text/html; charset=UTF-8").map_err(|e| AppError::OAuth(format!("Invalid callback header: {e:?}")))?));
  if params.get("state")!=Some(&state){return Err(AppError::OAuth("State validation failed".into()));} if let Some(err)=params.get("error"){return Err(AppError::OAuth(err.clone()));} let code=params.get("code").ok_or_else(||AppError::OAuth("No authorization code returned".into()))?;
  let token_url=cfg.token_uri.unwrap_or_else(||TOKEN_URL.into());
  let mut form: Vec<(&str, &str)> = vec![
      ("code", code.as_str()),
      ("client_id", cfg.client_id.as_str()),
      ("redirect_uri", redirect.as_str()),
      ("grant_type", "authorization_code"),
      ("code_verifier", verifier.as_str()),
  ];
  if let Some(secret) = cfg.client_secret.as_deref() {
      form.push(("client_secret", secret));
  }
  let req=crate::http::client().post(token_url).form(&form); let token:TokenResponse=req.send().await.map_err(|e|AppError::Network(e.to_string()))?.error_for_status().map_err(|e|AppError::OAuth(e.to_string()))?.json().await.map_err(|e|AppError::OAuth(e.to_string()))?;
  let refresh=token.refresh_token.ok_or_else(||AppError::OAuth("Google did not return a refresh token. Try signing out of the app's Google consent and sign in again.".into()))?; storage::save_refresh_token(&refresh)?; self.access_token=Some(token.access_token); self.expires_at=Some(Instant::now()+Duration::from_secs(token.expires_in.saturating_sub(60))); self.state=AuthState{authenticated:true,..Default::default()}; Ok(self.state.clone())
 }
 pub async fn access_token(&mut self, app:&tauri::AppHandle)->Result<String,AppError>{if let (Some(t),Some(exp))=(&self.access_token,self.expires_at){if Instant::now() < exp { return Ok(t.clone()); }} self.refresh_access_token(app).await?;self.access_token.clone().ok_or(AppError::AuthRequired)}
 async fn refresh_access_token(&mut self, app:&tauri::AppHandle)->Result<(),AppError>{
  let refresh=storage::get_refresh_token()?.ok_or(AppError::AuthRequired)?;
  let cfg=client_config(app)?;
  let token_url=cfg.token_uri.unwrap_or_else(||TOKEN_URL.into());
  let mut form: Vec<(&str, &str)> = vec![
      ("client_id", cfg.client_id.as_str()),
      ("refresh_token", refresh.as_str()),
      ("grant_type", "refresh_token"),
  ];
  if let Some(secret) = cfg.client_secret.as_deref() {
      form.push(("client_secret", secret));
  }
  let req=crate::http::client().post(token_url).form(&form);
  let response=req.send().await.map_err(|e|AppError::Network(e.to_string()))?;
  if response.status()==reqwest::StatusCode::BAD_REQUEST {
   let body=response.text().await.unwrap_or_default();
   if body.contains("invalid_grant") {
    let _=storage::delete_refresh_token();
    self.access_token=None; self.expires_at=None; self.state.authenticated=false;
    return Err(AppError::AuthRequired);
   }
   return Err(AppError::OAuth(format!("Google token refresh failed: {body}")));
  }
  let token:TokenResponse=response.error_for_status().map_err(|e|AppError::OAuth(e.to_string()))?.json().await.map_err(|e|AppError::OAuth(e.to_string()))?;
  self.access_token=Some(token.access_token);
  self.expires_at=Some(Instant::now()+Duration::from_secs(token.expires_in.saturating_sub(60)));
  self.state.authenticated=true;
  Ok(())
 }
 pub fn sign_out(&mut self)->Result<(),AppError>{storage::delete_refresh_token()?;self.access_token=None;self.expires_at=None;self.state=AuthState::default();Ok(())}
}
