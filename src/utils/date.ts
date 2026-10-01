import type { CalendarEvent } from '../types/calendar';

const pad = (value: number) => String(value).padStart(2, '0');

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function parseLocalDate(value: string): Date {
  const [year, month, day] = value.split('-').map(Number);
  return new Date(year, month - 1, day);
}

export function monthKey(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}`;
}

export function calendarGrid(month: Date, firstDayOfWeek: 0 | 1): Date[] {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  const offset = (first.getDay() - firstDayOfWeek + 7) % 7;
  const start = new Date(first.getFullYear(), first.getMonth(), 1 - offset);
  return Array.from({ length: 42 }, (_, index) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + index));
}

export function calendarRange(month: Date, firstDayOfWeek: 0 | 1): { start: string; end: string } {
  const days = calendarGrid(month, firstDayOfWeek);
  const start = dateKey(days[0]);
  const last = days[41];
  const end = dateKey(new Date(last.getFullYear(), last.getMonth(), last.getDate() + 1));
  return { start: `${start}T00:00:00`, end: `${end}T00:00:00` };
}

function eventStart(event: CalendarEvent): string {
  return event.start.dateTime || event.start.date || '';
}

function eventEnd(event: CalendarEvent): string {
  return event.end.dateTime || event.end.date || '';
}

export function eventOverlapsDay(event: CalendarEvent, day: Date): boolean {
  const key = dateKey(day);
  const next = dateKey(new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1));
  if (event.start.date) return event.start.date < next && (event.end.date || '') > key;

  const start = event.start.dateTime ? new Date(event.start.dateTime).getTime() : NaN;
  const end = event.end.dateTime ? new Date(event.end.dateTime).getTime() : NaN;
  const dayStart = new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime();
  const dayEnd = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1).getTime();
  return Number.isFinite(start) && Number.isFinite(end) && start < dayEnd && end > dayStart;
}

export function eventsForDay(events: CalendarEvent[], day: Date): CalendarEvent[] {
  return events.filter(event => eventOverlapsDay(event, day));
}

export function eventDisplayTime(event: CalendarEvent, timeFormat: '12h' | '24h', timeZone?: string): string {
  if (!event.start.dateTime) return '';
  return new Intl.DateTimeFormat('pt-BR', {
    hour: '2-digit', minute: '2-digit', hour12: timeFormat === '12h', ...(timeZone ? { timeZone } : {})
  }).format(new Date(event.start.dateTime));
}

export function formatFullDate(date: Date): string {
  return new Intl.DateTimeFormat('pt-BR', { weekday: 'long', day: 'numeric', month: 'long' }).format(date);
}

export function formatMonth(date: Date): string {
  return new Intl.DateTimeFormat('pt-BR', { month: 'long', year: 'numeric' }).format(date);
}

export function relativeDateLabel(date: Date, now = new Date()): string {
  const a = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const b = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const diffDays = Math.round((b.getTime() - a.getTime()) / 86400000);
  if (diffDays === 0) return 'Hoje';
  if (diffDays === 1) return 'Amanhã';
  if (diffDays === -1) return 'Ontem';
  const abs = Math.abs(diffDays);
  const weeks = Math.floor(abs / 7);
  const days = abs % 7;
  const parts: string[] = [];
  if (weeks) parts.push(`${weeks} ${weeks === 1 ? 'semana' : 'semanas'}`);
  if (days) parts.push(`${days} ${days === 1 ? 'dia' : 'dias'}`);
  const value = parts.join(' e ');
  return diffDays > 0 ? `em ${value}` : `há ${value}`;
}

export function relativeDateText(date: Date, now = new Date()): string {
  const label = relativeDateLabel(date, now);
  return label === 'Hoje' ? 'Hoje' : `Hoje | ${label.toLowerCase()}`;
}

export function eventKey(event: Pick<CalendarEvent, 'calendarId' | 'id'>): string {
  return `${event.calendarId}:${event.id}`;
}
