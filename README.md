# Calendar Gadget

A lightweight Windows 11 desktop calendar widget built with Tauri 2, React, TypeScript, Vite, Rust, and the Google Calendar API v3.

This project is intentionally single-user and local. There is no backend, registration, SaaS account system, telemetry, analytics, or subscription layer.

## What it does

- Compact frameless monthly calendar widget.
- Light, dark, and system themes.
- Draggable/resizable window with saved position and size; maximize is intentionally disabled to keep gadget behavior.
- Optional always-on-top, Windows startup, and start-hidden behavior.
- System tray show/hide/quit controls.
- Single-instance behavior.
- Google OAuth 2.0 Authorization Code flow with PKCE and a loopback redirect on `127.0.0.1:<random-port>`.
- Google Calendar list and calendar selection.
- One Google account in the MVP; auth/storage boundaries are isolated for later multi-account support.
- Cached events for offline viewing.
- Full create, edit, and delete support for normal Google Calendar events.
- All-day, timed, multi-day, and recurring event instances returned by Google Calendar.
- Incremental event synchronization using Google Calendar sync tokens, with automatic recovery when a sync token expires.
- Calendar and event colors when provided by Google.
- No refresh/access tokens exposed to the React frontend.

## Requirements on Windows 11 x64

Install:

1. Node.js 22+ (current LTS is fine).
2. Rust with Cargo via rustup.
3. Microsoft C++ Build Tools with the Desktop development with C++ workload.
4. WebView2 Runtime (normally already present on Windows 11).
5. Git, optional but recommended.

Tauri's current documentation requires Rust 1.90.0 or newer for Tauri 2.12. Keep Rust updated with `rustup update`. See the official Tauri documentation for platform setup.

## 1. Google Cloud setup

The project uses a desktop OAuth client, not a web-client credential.

Google's current documentation places OAuth configuration under **Google Auth platform** in Google Cloud Console. For a desktop application, create an OAuth client with application type **Desktop app**. Google documents the desktop loopback redirect and PKCE flow for Windows desktop applications.

### Create the project

1. Open Google Cloud Console: https://console.cloud.google.com/
2. Create a new project, or use a dedicated personal project.
3. Select that project.

### Enable Google Calendar API

1. Open **APIs & Services → Library**.
2. Search for **Google Calendar API**.
3. Enable it.

### Configure OAuth / consent

Google's UI has changed over time. The current console uses **Google Auth platform**.

For a purely personal application, use the simplest user-audience configuration available to your Google account. If Google asks for an audience and the account is not inside a Google Workspace organization where an Internal audience is applicable, use **External** and add your own Google account as a test user during development.

You only need calendar-related permissions. This application does not ask for Gmail, Drive, Contacts, or unrelated Google APIs.

### Create the desktop OAuth client

1. In Google Cloud Console, open **Google Auth platform → Clients**.
2. Click **Create Client**.
3. Choose **Desktop app** as the application type.
4. Give it a name such as `Calendar Gadget`.
5. Create the client.
6. Download the JSON credentials file.

The current Google documentation describes this exact Desktop app credential type and loopback redirect flow for Windows desktop applications.

### Put the credential in the project

Rename the downloaded JSON file to:

`google-client.json`

Then place it here:

```text
calendar-widget/
└── src-tauri/
    └── google-client.json
```

Do not put it in `src/` and do not paste it into React code.

The file is explicitly ignored by Git. The app reads it from the Rust/Tauri side only.

A template is provided at:

`src-tauri/google-client.example.json`

Do **not** commit your real `google-client.json`.

## 2. Install dependencies

From the project directory:

```powershell
npm install
```

Then:

```powershell
cd src-tauri
cargo check
cd ..
```

The first Cargo build may take several minutes because Rust downloads and compiles native dependencies.

## 3. Run in development

```powershell
npm run tauri dev
```

The app should open as a compact desktop window.

Click **Sign in with Google**. Your normal system browser opens Google's authorization page. After consent, Google redirects to a random localhost port owned by the running desktop application. No backend server is involved.

## 4. Build a Windows application

```powershell
npm run tauri build
```

Tauri will place the generated Windows installer and application artifacts under `src-tauri/target/release/bundle/`.

## OAuth security model

The OAuth implementation follows Google's installed-app model:

- Authorization Code flow.
- PKCE with S256.
- Random state validation.
- Random loopback port on `127.0.0.1`.
- No OAuth tokens in URLs after the callback is processed.
- Access tokens exist only in Rust memory and expire normally.
- The refresh token is stored with the Windows-native credential backend through the `keyring` crate.
- React invokes Rust commands instead of receiving refresh tokens.
- Tokens are never written to the JSON cache.
- Tokens are never written to application logs.
- Signing out deletes the locally stored refresh token and clears the local event cache.

Google's desktop OAuth documentation explicitly treats installed apps as unable to keep a client secret confidential. The application therefore keeps the credential file on the Rust side and never places its contents in frontend JavaScript, while still treating user tokens as sensitive secrets.

## Permissions

The application requests only:

- `https://www.googleapis.com/auth/calendar.events` — create, edit, and delete calendar events.
- `https://www.googleapis.com/auth/calendar.calendarlist.readonly` — list the user's calendars so the widget can let the user choose which calendars to display.

No Gmail, Drive, Contacts, or other Google scopes are requested.

## Synchronization model

Google Calendar remains the source of truth. The local JSON cache is only a performance/offline cache.

The initial synchronization for each selected calendar performs a complete `events.list` request with `singleEvents=true` and stores Google's `nextSyncToken`. This is intentional: Google requires incremental requests to preserve the same query shape, and `timeMin`/`timeMax` cannot be used with `syncToken`. The local cache can contain events outside the currently displayed month; the UI filters them for display.

Later synchronizations use the saved sync token and request only changes. Deleted events are returned by Google and removed from the local cache. If Google returns HTTP 410 because the sync token is no longer valid, the application clears that calendar's cached events/token and performs a new full synchronization.

Google documents these synchronization constraints in its Calendar API synchronization guide.

## Time zones

Timed event data is kept with the Google `dateTime` and `timeZone` fields. New and edited timed events use the computer's current IANA time zone from the browser runtime. All-day events use Google Calendar's date-only representation and the exclusive end-date convention.

The UI does not hardcode Brazil. Brazilian and non-Brazilian time zones are handled by the runtime and Google event data.

## Project structure

```text
src/
  components/     calendar grid and window title bar
  dialogs/        event creation/edit dialog
  pages/          settings panel
  services/       Tauri command API abstraction
  types/          shared frontend types
  utils/          date/calendar logic and tests
  styles/         application CSS
src-tauri/
  src/
    auth.rs       OAuth, PKCE, token lifecycle
    calendar.rs   Google Calendar API client and sync logic
    error.rs      application errors
    models.rs     Rust command/domain models
    storage.rs    JSON cache/settings + Windows keyring access
    sync.rs       synchronization command facade
    main.rs       Tauri commands, tray, window behavior
  capabilities/  Tauri permissions
  icons/         Windows application/tray icons
```

The React layer does not contain the OAuth protocol implementation or raw Google REST calls. Read-only calendars remain selectable for display, but event creation/editing is limited by the calendar access role exposed by Google.

## Tests

The nontrivial logic has tests for:

- Six-week calendar-grid calculation.
- All-day multi-day events.
- Timed events crossing midnight.
- Explicit IANA time-zone conversion.
- Incremental cache replacement/add/remove semantics in Rust.

Run them with:

```powershell
npm test
```

## Validation notes

The source in this repository was syntax-checked with the TypeScript compiler API, and the date/month/time-zone logic was exercised with deterministic tests. JSON configuration files were parsed successfully.

A full `npm install`/Tauri build could not be completed in the provided build environment because external package downloads failed at DNS/network resolution, and that environment does not contain a Rust/Cargo toolchain. This is an environment limitation, not a successful application build claim. On Windows 11, run the commands in the setup section to perform the real Tauri compilation.

## Troubleshooting

### `npm install` hangs or times out

Check that the Windows machine can resolve and reach npm's registry. The application itself does not need a custom package mirror.

### `google-client.json` is missing

Make sure the downloaded Desktop OAuth client JSON is at exactly:

`src-tauri/google-client.json`

### Google says the app is not verified

For a personal development application using sensitive Google data, Google may show a testing/unverified warning. Make sure the account you are using is configured as an allowed test user when the OAuth consent configuration requires it.

### Sign-in finishes but the app says authentication is required

Sign out from the widget and authenticate again. If the stored refresh token was revoked, the application removes the invalid local token and requires a new authorization.

### Events disappear while offline

The cache is local, but it is intentionally not treated as authoritative. The widget continues displaying cached events when synchronization cannot reach Google. Signing out deliberately clears the cache. A later successful synchronization reconciles cached data with Google.

### Build fails in Rust

Run:

```powershell
rustup update
cd src-tauri
cargo check
```

Then run `npm run tauri dev` again. Native Windows builds also require the Microsoft C++ build tools.

## Development sequence

The codebase is organized so the implementation can be worked on in the following order without redesigning the architecture:

1. UI and local date logic.
2. OAuth and secure token storage.
3. Calendar list/read operations.
4. Event mutations.
5. Incremental synchronization and cache reconciliation.
6. Windows tray/window behavior.
7. UI polish and accessibility improvements.

## Official documentation

Google desktop OAuth: https://developers.google.com/identity/protocols/oauth2/native-app

Google Calendar API authorization: https://developers.google.com/workspace/calendar/api/auth

Google Calendar events list: https://developers.google.com/workspace/calendar/api/v3/reference/events/list

Google Calendar API quickstart / credentials: https://developers.google.com/workspace/guides/create-credentials

Tauri 2: https://v2.tauri.app/
## Nota sobre single-instance

O plugin `tauri-plugin-single-instance` é usado no backend Rust. Este projeto não usa bindings JavaScript desse plugin, então `@tauri-apps/plugin-single-instance` não deve ser instalado via npm.

