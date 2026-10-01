# Validation

## Verified in source

- Vitest date/timezone test suite: 4 tests passed in the prior Windows validation run.
- TypeScript build issue reported by Windows was traced to two source issues and fixed:
  - added `src/vite-env.d.ts` to load Vite client typings for CSS imports;
  - added `getCachedCalendars()` to `src/services/tauriApi.ts` for the existing Rust command `get_cached_calendars`.
- Google OAuth client file is excluded from the project archive; copy the local `src-tauri/google-client.json` separately.

## Not verified in this environment

- Full `npm install` / Vite build in this Linux container because external package installation timed out.
- Native Windows Tauri/Rust build because the Windows Rust/MSVC toolchain is not available here.


### Windows Tauri dev fix (2026-09-30)
- Pinned the JS opener/autostart plugin versions to the same major/minor releases as their Rust crates to avoid Tauri's version-mismatch warning.
- Updated Vite's file watcher to ignore `src-tauri/**`; this prevents Windows `EBUSY` errors when Cargo compiles and locks build-script `.exe` files under `src-tauri/target`.


## Rust Windows fixes

The source was patched after Windows/Tauri compilation exposed six compile errors: the tray icon path, tiny_http `Content-Type` header typing, `Instant` comparison, `page_token` inference, missing `tauri::Manager` import, and `std::io::Error` pattern matching. These fixes do not add dependencies.
