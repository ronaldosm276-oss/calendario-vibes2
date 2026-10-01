import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CalendarGrid } from './components/CalendarGrid';
import { TitleBar } from './components/TitleBar';
import { EventDialog } from './dialogs/EventDialog';
import { SettingsPanel } from './pages/SettingsPanel';
import { SelectedDayPopover } from './components/SelectedDayPopover';
import { api } from './services/tauriApi';
import type { AlarmInfo, AuthState, CalendarEvent, CalendarInfo, ReminderRule, Settings, SyncState } from './types/calendar';
import { calendarRange, eventKey, eventsForDay, monthKey, parseLocalDate, relativeDateText } from './utils/date';
import { DEFAULT_EVENT_COLOR, eventColorKey, normalizeHex, readableText, resolveEventColor } from './utils/colors';
import './styles/app.css';
import { listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';

const defaultSettings: Settings = {
  theme: 'system', alwaysOnTop: false, startWithWindows: false, startHidden: false,
  rememberPosition: true, rememberSize: true, opacity: 0.96, visibleCalendarIds: [],
  firstDayOfWeek: 1, timeFormat: '24h', automaticSync: true, syncIntervalMinutes: 15,
  selectedMonth: '', calendarColors: {}, eventColors: {}, fontScale: 1, density: 'comfortable', zoom: 1, accentColor: null,
  remindersEnabled: true, defaultReminderEnabled: true, defaultReminderMinutes: 10, defaultReminderSound: null, alarmVolume: 0.9, snoozeMinutes: 5, reminders: {}
};

function monthFromKey(value: string): Date | null {
  if (!/^\d{4}-\d{2}$/.test(value)) return null;
  const [year, month] = value.split('-').map(Number);
  const date = new Date(year, month - 1, 1);
  return date.getFullYear() === year && date.getMonth() === month - 1 ? date : null;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isOfflineError(message: string): boolean {
  return /network|timed out|timeout|connection|dns|offline|fetch/i.test(message);
}

type SyncReason = 'start' | 'interval' | 'manual' | 'calendars' | 'after-save' | 'after-delete';

// Module-level counter so the console shows WHO triggered each call (P0-0 instrumentation).
let syncCallCount = 0;
console.debug('[calendar] App module evaluated', performance.getEntriesByType('navigation').map(n => (n as PerformanceNavigationTiming).type));

export default function App() {
  const [month, setMonth] = useState(() => new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const [selectedDay, setSelectedDay] = useState(() => new Date());
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [settingsReady, setSettingsReady] = useState(false);
  const [auth, setAuth] = useState<AuthState>({ authenticated: false });
  const [calendars, setCalendars] = useState<CalendarInfo[]>([]);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [sync, setSync] = useState<SyncState>({ status: 'idle' });
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [dayPopoverOpen, setDayPopoverOpen] = useState(false);
  const [eventDialog, setEventDialog] = useState<{ event?: CalendarEvent; date: Date } | null>(null);
  const [error, setError] = useState('');

  // Refs hold the latest values for callbacks/intervals so they never read a stale closure.
  const monthRef = useRef(month);
  const settingsRef = useRef(settings);
  const authRef = useRef(auth);
  const calendarsRef = useRef(calendars);
  const eventsRef = useRef(events);
  monthRef.current = month;
  settingsRef.current = settings;
  authRef.current = auth;
  calendarsRef.current = calendars;
  eventsRef.current = events;

  const displaySeq = useRef(0);        // discards out-of-order cache reads
  const syncingRef = useRef(false);    // only one network sync at a time
  const queuedReasonRef = useRef<SyncReason | null>(null); // explicit follow-up requested while a sync is running
  const pendingSave = useRef<Promise<unknown>>(Promise.resolve());
  const lastCalendarSyncKey = useRef<string | null>(null);

  const visibleIds = settings.visibleCalendarIds ?? [];
  const visibleKey = visibleIds.join('|');
  const monthId = monthKey(month);
  const syncEveryMs = Math.max(1, Number(settings.syncIntervalMinutes) || 15) * 60_000;

  const visibleCalendars = useMemo(
    () => calendars.filter(c => visibleIds.includes(c.id)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [calendars, visibleKey]
  );

  // 1) Boot: load persisted settings + auth once.
  useEffect(() => {
    (async () => {
      try {
        const [savedSettings, savedAuth] = await Promise.all([api.getSettings(), api.getAuthState()]);
        const restoredSettings: Settings = {
          ...defaultSettings,
          ...savedSettings,
          visibleCalendarIds: Array.isArray(savedSettings?.visibleCalendarIds) ? savedSettings.visibleCalendarIds : [],
          reminders: savedSettings?.reminders && typeof savedSettings.reminders === 'object' ? savedSettings.reminders : {},
        };
        setSettings(restoredSettings);
        setAuth(savedAuth);
        const restoredMonth = monthFromKey(restoredSettings.selectedMonth);
        if (restoredMonth) setMonth(restoredMonth);
      } catch (e) {
        setError(errorText(e));
      } finally {
        setSettingsReady(true);
      }
    })();
  }, []);

  // 2) Persist settings. The promise is remembered so a sync never reads stale settings.json.
  useEffect(() => {
    if (!settingsReady) return;
    const root = document.documentElement;
    root.dataset.theme = settings.theme;
    root.dataset.density = settings.density ?? 'comfortable';
    root.style.setProperty('--calendar-font-scale', '1');
    root.style.setProperty('--calendar-scale', String(settings.zoom ?? 1));
    const accent = normalizeHex(settings.accentColor);
    if (accent) {
      root.style.setProperty('--accent', accent);
      root.style.setProperty('--accent-soft', `color-mix(in srgb, ${accent} 18%, transparent)`);
      root.style.setProperty('--accent-fg', readableText(accent));
    } else {
      ['--accent', '--accent-soft', '--accent-fg'].forEach(p => root.style.removeProperty(p));
    }
    console.debug('[calendar] saveSettings');
    pendingSave.current = api.saveSettings(settings).catch(e => setError(errorText(e)));
  }, [settings, settingsReady]);

  useEffect(() => {
    if (!settingsReady) return;
    setSettings(current => current.selectedMonth === monthId ? current : { ...current, selectedMonth: monthId });
  }, [monthId, settingsReady]);

  // Ctrl + / Ctrl - / Ctrl 0: escala persistida do calendário.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (e.key === '0') { e.preventDefault(); setSettings(c => ({ ...c, zoom: 1 })); return; }
      const step = e.key === '+' || e.key === '=' ? 0.1 : e.key === '-' ? -0.1 : 0;
      if (!step) return;
      e.preventDefault();
      setSettings(c => ({ ...c, zoom: Math.min(1.5, Math.max(0.7, Math.round(((c.zoom ?? 1) + step) * 10) / 10)) }));
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // 3) DISPLAY: read the local cache for the visible month. No network. Runs on month change only.
  const showCached = useCallback(async (reason: string) => {
    const seq = ++displaySeq.current;
    const range = calendarRange(monthRef.current, settingsRef.current.firstDayOfWeek);
    try {
      const cached = await api.getCachedEvents(range.start, range.end);
      if (seq !== displaySeq.current) return;           // a newer read superseded this one
      console.debug(`[calendar] showCached(${reason}) -> ${cached.length} events`);
      setEvents(cached);
    } catch (e) {
      if (seq === displaySeq.current) setSync({ status: 'error', message: errorText(e) });
    }
  }, []);

  useEffect(() => {
    if (!settingsReady) return;
    void showCached(`month ${monthId}`);
  }, [monthId, settingsReady, showCached]);

  // 4) SYNC: network. Single-flight; concurrent requests collapse into one follow-up run.
  const refresh = useCallback(async (reason: SyncReason) => {
    if (!authRef.current.authenticated) return;
    if (syncingRef.current) {
      // Do not let a 15-minute interval create a second sync behind a running one.
      // Explicit user/action-triggered refreshes are coalesced into one follow-up.
      if (reason !== 'interval') queuedReasonRef.current = reason;
      return;
    }
    syncingRef.current = true;
    const id = ++syncCallCount;
    console.debug(`[calendar] sync #${id} start (${reason})`);
    if (reason !== 'interval') setSync(prev => ({ ...prev, status: 'syncing' }));
    try {
      await pendingSave.current;   // make sure Rust sees the current visibleCalendarIds
      if (!calendarsRef.current.length) {
        try {
          const list = await api.listCalendars();
          setCalendars(list);
          calendarsRef.current = list;
          if (!(settingsRef.current.visibleCalendarIds ?? []).length) {
            const ids = list.map(c => c.id);
            const defaults = list.find(c => c.primary)?.id || list[0]?.id;
            // The sync below already uses these calendars. Mark the resulting
            // visible-calendar key as handled so the state update does not
            // immediately schedule a duplicate startup sync.
            lastCalendarSyncKey.current = `${authRef.current.authenticated}:${ids.join('|')}`;
            setSettings(current => ({ ...current, visibleCalendarIds: ids, defaultCalendarId: defaults }));
          }
        } catch (e) {
          const cachedList = await api.getCachedCalendars().catch(() => []);
          if (cachedList.length) setCalendars(cachedList);
          throw e;
        }
      }
      const result = await api.sync();
      console.debug(`[calendar] sync #${id} done`, result.message);
      setSync(result);
      await showCached(`after sync #${id}`);
    } catch (e) {
      const message = errorText(e);
      console.debug(`[calendar] sync #${id} failed`, message);
      setSync({ status: /authentication required/i.test(message) ? 'auth-required' : isOfflineError(message) ? 'offline' : 'error', message });
    } finally {
      syncingRef.current = false;
      const queuedReason = queuedReasonRef.current;
      queuedReasonRef.current = null;
      if (queuedReason) void refresh(queuedReason);
    }
  }, [showCached]);

  // Sync on start / sign-in and when the set of visible calendars changes. NOT on month change.
  useEffect(() => {
    if (!auth.authenticated || !settingsReady) return;
    const key = `${auth.authenticated}:${visibleKey}`;
    if (lastCalendarSyncKey.current === key) return;
    lastCalendarSyncKey.current = key;
    void refresh(visibleKey ? 'calendars' : 'start');
  }, [auth.authenticated, settingsReady, visibleKey, refresh]);

  useEffect(() => {
    if (!auth.authenticated || !settings.automaticSync || !settingsReady) return;
    console.debug(`[calendar] interval set to ${syncEveryMs} ms`);
    const timer = window.setInterval(() => void refresh('interval'), syncEveryMs);
    return () => window.clearInterval(timer);
  }, [auth.authenticated, settings.automaticSync, settingsReady, syncEveryMs, refresh]);

  async function signIn() {
    try {
      const account = await api.signIn();
      setAuth(account);
      setSettingsOpen(false);
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function signOut() {
    try {
      await api.signOut();
      setAuth({ authenticated: false });
      lastCalendarSyncKey.current = null;
      setCalendars([]);
      setEvents([]);
      setSync({ status: 'idle' });
    } catch (e) {
      setError(errorText(e));
    }
  }

  async function saveEvent(event: CalendarEvent, color: string | null, reminders: ReminderRule[]) {
    try {
      const saved = event.id ? await api.updateEvent(event) : await api.createEvent(event);
      setSettings(cur => {
        const colors = { ...(cur.eventColors ?? {}) };
        const key = eventColorKey(saved);
        if (color) colors[key] = color; else delete colors[key];
        const reminderMap = { ...(cur.reminders ?? {}) };
        if (reminders.length) reminderMap[eventKey(saved)] = reminders;
        else delete reminderMap[eventKey(saved)];
        return { ...cur, eventColors: colors, reminders: reminderMap };
      });
      setEvents(old => event.id
        ? old.map(existing => existing.id === saved.id && existing.calendarId === saved.calendarId ? saved : existing)
        : [...old, saved]);
      await refresh('after-save');
    } catch (e) {
      setError(errorText(e));
      throw e;
    }
  }

  async function deleteEvent(event: CalendarEvent) {
    if (!window.confirm(`Excluir “${event.summary || '(Sem título)'}”?`)) return;
    try {
      await api.deleteEvent(event.calendarId, event.id);
      setEvents(old => old.filter(e => !(e.id === event.id && e.calendarId === event.calendarId)));
      setSettings(cur => { const reminders = { ...(cur.reminders ?? {}) }; delete reminders[eventKey(event)]; return { ...cur, reminders }; });
      setEventDialog(null);
      await refresh('after-delete');
    } catch (e) {
      setError(errorText(e));
    }
  }

  const colorForCalendar = (id: string) => normalizeHex(settings.calendarColors?.[id]) || normalizeHex(calendars.find(c => c.id === id)?.backgroundColor) || DEFAULT_EVENT_COLOR;
  const shownEvents = useMemo(() => events.filter(e => visibleIds.includes(e.calendarId)), [events, visibleKey]);
  const selectedDayEvents = useMemo(() => eventsForDay(shownEvents, selectedDay), [shownEvents, selectedDay]);
  const syncLabel = sync.status === 'syncing' ? 'Sincronizando' : sync.status === 'synced' ? 'Sincronizado' : sync.status === 'offline' ? 'Offline' : sync.status === 'auth-required' ? 'Autenticação necessária' : auth.authenticated ? 'Pronto' : 'Entre com sua conta';

  useEffect(() => {
    if (!settingsReady) return;
    let unlisten: (() => void) | undefined;
    void listen<AlarmInfo>('reminder-triggered', async ({ payload }) => {
      const body = payload.title ? `${payload.title} — agora` : 'Um lembrete está aguardando sua atenção.';
      try {
        let granted = await isPermissionGranted();
        if (!granted) granted = (await requestPermission()) === 'granted';
        if (granted) await sendNotification({ title: 'Lembrete', body, sound: 'default' });
      } catch (error) {
        console.debug('[calendar] notification failed', error);
      }
      const alarm = await WebviewWindow.getByLabel('alarm');
      if (alarm) {
        await alarm.show().catch(() => undefined);
        await alarm.setFocus().catch(() => undefined);
      }
    }).then(fn => { unlisten = fn; });
    return () => unlisten?.();
  }, [settingsReady]);

  useEffect(() => {
    if (!settingsReady) return;
    let unlisten: (() => void) | undefined;
    void listen<{ eventKey: string; startAt?: string }>('alarm-open-event', async ({ payload }) => {
      const event = eventsRef.current.find(item => eventKey(item) === payload.eventKey);
      if (event) {
        const day = event.start.date ? parseLocalDate(event.start.date) : new Date(event.start.dateTime || new Date().toISOString());
        setSelectedDay(day);
        setMonth(new Date(day.getFullYear(), day.getMonth(), 1));
        setDayPopoverOpen(false);
        setEventDialog({ event, date: day });
      } else {
        const day = new Date(payload.startAt || new Date().toISOString());
        if (Number.isFinite(day.getTime())) {
          setSelectedDay(day);
          setMonth(new Date(day.getFullYear(), day.getMonth(), 1));
          setDayPopoverOpen(true);
        }
      }
    }).then(fn => { unlisten = fn; });
    return () => unlisten?.();
  }, [settingsReady]);

  useEffect(() => {
    if (settingsReady) {
      document.title = 'Calendário';
    }
  }, [settingsReady]);

  const reminderText = relativeDateText(selectedDay);
  return <div className="app-shell">
    <TitleBar
      onSettings={() => setSettingsOpen(true)}
      onRefresh={() => void refresh('manual')}
      syncing={sync.status === 'syncing'}
      syncLabel={syncLabel}
    />
    <main>
      <div className="calendar-content-scale">
        <CalendarGrid
          month={month}
          events={shownEvents}
          settings={settings}
          onMonthChange={(nextMonth) => { setMonth(nextMonth); setDayPopoverOpen(false); }}
          onDay={(day) => { setSelectedDay(day); setDayPopoverOpen(true); }}
          selectedDay={selectedDay}
          onEvent={(event) => { const day = event.start.date ? parseLocalDate(event.start.date) : new Date(event.start.dateTime || selectedDay); setSelectedDay(day); setEventDialog({ event, date: day }); }}
          onCreate={(date) => setEventDialog({ date })}
          onToday={() => { const day = new Date(); setSelectedDay(day); setMonth(new Date(day.getFullYear(), day.getMonth(), 1)); setDayPopoverOpen(true); }}
        />
      </div>
    </main>
    {selectedDay && dayPopoverOpen && (
      <SelectedDayPopover
        date={selectedDay}
        relativeText={reminderText}
        events={selectedDayEvents}
        calendars={calendars}
        settings={settings}
        onClose={() => setDayPopoverOpen(false)}
        onEvent={(event) => { const day = event.start.date ? parseLocalDate(event.start.date) : new Date(event.start.dateTime || selectedDay); setSelectedDay(day); setEventDialog({ event, date: day }); }}
        onCreate={() => setEventDialog({ date: selectedDay })}
      />
    )}
    {!auth.authenticated && <div className="signin-hint"><button className="primary-button" onClick={() => void signIn()}>Entrar com Google</button><span>Os eventos salvos em cache continuam disponíveis offline.</span></div>}
    {settingsOpen && <SettingsPanel settings={settings} calendars={calendars} auth={auth} onChange={setSettings} onClose={() => setSettingsOpen(false)} onSignIn={() => void signIn()} onSignOut={() => void signOut()} onError={setError} />}
    {eventDialog && <EventDialog event={eventDialog.event} date={eventDialog.date} calendars={visibleCalendars.length ? visibleCalendars : calendars} defaultCalendarId={settings.defaultCalendarId} settings={settings} onClose={() => setEventDialog(null)} eventColor={eventDialog.event ? settings.eventColors?.[eventColorKey(eventDialog.event)] : undefined} colorFor={colorForCalendar} onSave={saveEvent} onDelete={eventDialog.event ? () => deleteEvent(eventDialog.event!) : undefined} />}
    {error && <div className="toast" role="status" onClick={() => setError('')}>{error}</div>}
  </div>;
}
