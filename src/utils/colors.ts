import type { CalendarEvent, Settings } from '../types/calendar';

export const DEFAULT_EVENT_COLOR = '#2f63c7';

export function normalizeHex(value?: string | null): string | null {
  if (!value) return null;
  const v = value.trim();
  if (/^#[0-9a-f]{6}$/i.test(v)) return v.toLowerCase();
  const m = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(v);
  return m ? `#${m[1]}${m[1]}${m[2]}${m[2]}${m[3]}${m[3]}`.toLowerCase() : null;
}

function channel(c: number) {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}
export function luminance(hex: string) {
  const n = parseInt(hex.slice(1), 16);
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}
export function contrast(a: string, b: string) {
  const la = luminance(a), lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}
/** Black or white, whichever has the higher WCAG contrast on the given background. */
export function readableText(bg: string): '#000000' | '#ffffff' {
  const h = normalizeHex(bg) ?? DEFAULT_EVENT_COLOR;
  return contrast(h, '#000000') >= contrast(h, '#ffffff') ? '#000000' : '#ffffff';
}

/** A recurring series shares one color override (keyed by the series id). */
export const eventColorKey = (e: Pick<CalendarEvent, 'calendarId' | 'id' | 'recurringEventId'>) =>
  `${e.calendarId}:${e.recurringEventId || e.id}`;

/** Precedence: per-event (local) > per-calendar (local) > Google event color > Google calendar color > accent. */
export function resolveEventColor(event: CalendarEvent, settings: Settings): string {
  return normalizeHex(settings.eventColors?.[eventColorKey(event)])
    ?? normalizeHex(settings.calendarColors?.[event.calendarId])
    ?? normalizeHex(event.eventColor)
    ?? normalizeHex(event.calendarColor)
    ?? normalizeHex(settings.accentColor)
    ?? DEFAULT_EVENT_COLOR;
}
