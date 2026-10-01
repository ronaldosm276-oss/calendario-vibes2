import { describe, expect, it } from 'vitest';
import { calendarGrid, calendarRange, dateKey, eventDisplayTime, eventOverlapsDay, eventsForDay } from '../date';
import type { CalendarEvent } from '../../types/calendar';

describe('calendar date utilities', () => {
  it('creates a six-week grid beginning on the requested weekday', () => {
    const grid = calendarGrid(new Date(2026, 8, 1), 1);
    expect(grid).toHaveLength(42);
    expect(dateKey(grid[0])).toBe('2026-08-31');
    expect(dateKey(grid[35])).toBe('2026-10-05');
  });

  it('builds the cache range from the same 42 cells rendered by October 2026', () => {
    const grid = calendarGrid(new Date(2026, 9, 1), 1);
    const range = calendarRange(new Date(2026, 9, 1), 1);
    expect(dateKey(grid[0])).toBe('2026-09-28');
    expect(dateKey(grid[2])).toBe('2026-09-30');
    expect(range.start).toBe('2026-09-28T00:00:00Z');
    expect(range.end).toBe('2026-11-09T00:00:00Z');
  });

  it('maps an all-day multi-day event to every overlapping day', () => {
    const event: CalendarEvent = { id:'1', calendarId:'c', summary:'Trip', start:{date:'2026-09-10'}, end:{date:'2026-09-13'} };
    expect(eventOverlapsDay(event, new Date(2026,8,10))).toBe(true);
    expect(eventOverlapsDay(event, new Date(2026,8,12))).toBe(true);
    expect(eventOverlapsDay(event, new Date(2026,8,13))).toBe(false);
    expect(eventsForDay([event], new Date(2026,8,11))).toHaveLength(1);
  });

  it('maps timed events crossing midnight to both dates', () => {
    const event: CalendarEvent = { id:'2', calendarId:'c', summary:'Night', start:{dateTime:'2026-09-10T23:30:00-03:00'}, end:{dateTime:'2026-09-11T01:00:00-03:00'} };
    expect(eventOverlapsDay(event, new Date(2026,8,10))).toBe(true);
    expect(eventOverlapsDay(event, new Date(2026,8,11))).toBe(true);
  });
});


describe('event time zone formatting', () => {
  it('converts an event instant into the requested local time zone', () => {
    const event: CalendarEvent = {
      id: 'tz',
      calendarId: 'c',
      summary: 'Timezone',
      start: { dateTime: '2026-09-11T02:30:00+09:00' },
      end: { dateTime: '2026-09-11T03:30:00+09:00' }
    };
    expect(eventDisplayTime(event, '24h', 'America/Sao_Paulo')).toBe('14:30');
  });
});

