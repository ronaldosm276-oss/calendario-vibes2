export type ThemeMode = 'light' | 'dark' | 'system' | 'contrast';
export type SyncStatus = 'idle' | 'syncing' | 'synced' | 'offline' | 'auth-required' | 'error';

export interface CalendarInfo {
  id: string;
  summary: string;
  description?: string;
  timeZone?: string;
  backgroundColor?: string;
  foregroundColor?: string;
  primary?: boolean;
  accessRole?: string;
}

export interface CalendarEvent {
  id: string;
  calendarId: string;
  summary: string;
  description?: string;
  location?: string;
  status?: string;
  colorId?: string;
  eventColor?: string;
  calendarColor?: string;
  htmlLink?: string;
  start: { date?: string; dateTime?: string; timeZone?: string };
  end: { date?: string; dateTime?: string; timeZone?: string };
  recurringEventId?: string;
  originalStartTime?: { date?: string; dateTime?: string; timeZone?: string };
}

export interface ReminderRule {
  minutesBefore: number;
  sound?: string | null;
  enabled: boolean;
}

export interface AlarmInfo {
  id: string;
  eventKey: string;
  calendarId: string;
  eventId: string;
  title: string;
  startAt: string;
  sound?: string | null;
  snoozeMinutes: number;
}

export interface SoundInfo {
  name: string;
  path: string;
}

export interface EventDraft {
  calendarId: string;
  title: string;
  date: string;
  endDate: string;
  startTime: string;
  endTime: string;
  allDay: boolean;
  location: string;
  description: string;
}

export interface Settings {
  theme: ThemeMode;
  alwaysOnTop: boolean;
  startWithWindows: boolean;
  startHidden: boolean;
  rememberPosition: boolean;
  rememberSize: boolean;
  opacity: number;
  defaultCalendarId?: string;
  visibleCalendarIds: string[];
  firstDayOfWeek: 0 | 1;
  timeFormat: '12h' | '24h';
  automaticSync: boolean;
  syncIntervalMinutes: number;
  selectedMonth: string;
  calendarColors: Record<string, string>;
  eventColors: Record<string, string>;
  fontScale: number;
  density: 'compact' | 'comfortable';
  zoom: number;
  accentColor?: string | null;
  remindersEnabled: boolean;
  defaultReminderEnabled: boolean;
  defaultReminderMinutes: number;
  defaultReminderSound?: string | null;
  alarmVolume: number;
  snoozeMinutes: number;
  reminders: Record<string, ReminderRule[]>;
}

export interface AuthState {
  authenticated: boolean;
  email?: string;
  name?: string;
  picture?: string;
}

export interface SyncState {
  status: SyncStatus;
  lastSyncedAt?: string;
  message?: string;
}
