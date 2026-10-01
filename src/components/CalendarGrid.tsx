import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import type { CalendarEvent, Settings } from '../types/calendar';
import { calendarGrid, dateKey, eventDisplayTime, eventsForDay, formatMonth, monthKey } from '../utils/date';
import { resolveEventColor } from '../utils/colors';

interface Props {
  month: Date;
  events: CalendarEvent[];
  settings: Settings;
  onMonthChange: (month: Date) => void;
  onDay: (day: Date) => void;
  selectedDay: Date;
  onEvent: (event: CalendarEvent) => void;
  onCreate: (day: Date) => void;
  onToday: () => void;
}

const weekdayNames = ['dom.', 'seg.', 'ter.', 'qua.', 'qui.', 'sex.', 'sáb.'];

export function CalendarGrid({ month, events, settings, onMonthChange, onDay, selectedDay, onEvent, onCreate, onToday }: Props) {
  const days = useMemo(() => calendarGrid(month, settings.firstDayOfWeek), [month, settings.firstDayOfWeek]);
  const todayKey = dateKey(new Date());
  const selectedKey = dateKey(selectedDay);
  const gridRef = useRef<HTMLDivElement>(null);
  const [maxVisibleEvents, setMaxVisibleEvents] = useState(2);
  const monthId = monthKey(month);

  const eventsByDay = useMemo(() => {
    const map = new Map<string, CalendarEvent[]>();
    for (const day of days) map.set(dateKey(day), eventsForDay(events, day));
    return map;
  }, [days, events]);

  useEffect(() => {
    const element = gridRef.current;
    if (!element) return;
    const update = () => {
      const rect = element.getBoundingClientRect();
      const cellHeight = rect.height / 6;
      const compact = settings.density === 'compact';
      const scale = Math.max(0.7, Math.min(1.5, settings.zoom ?? 1));
      const rowHeight = (compact ? 19 : 20) * scale;
      const rowGap = 2 * scale;
      const reserved = 31 * scale;
      const available = Math.max(0, cellHeight - reserved);
      const fit = Math.floor((available + rowGap) / (rowHeight + rowGap));
      setMaxVisibleEvents(Math.max(1, Math.min(4, fit)));
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, [settings.density, settings.zoom, days.length]);

  const labels = settings.firstDayOfWeek === 1 ? [...weekdayNames.slice(1), weekdayNames[0]] : weekdayNames;
  const shift = (amount: number) => onMonthChange(new Date(month.getFullYear(), month.getMonth() + amount, 1));
  const heading = formatMonth(month);

  return <section className="calendar-panel" aria-label="Calendário mensal">
    <header className="calendar-header">
      <button className="month-nav" title="Mês anterior" aria-label="Mês anterior" onClick={() => shift(-1)}><ChevronLeft size={18} /></button>
      <div className="month-heading"><div className="month-title">{heading}</div><button className="today-button" onClick={onToday}>Hoje</button></div>
      <button className="month-nav" title="Próximo mês" aria-label="Próximo mês" onClick={() => shift(1)}><ChevronRight size={18} /></button>
    </header>

    <div className="weekday-row" aria-hidden="true">{labels.map(name => <div key={name}>{name}</div>)}</div>
    <div className="calendar-grid" role="grid" aria-label={heading} ref={gridRef} data-month={monthId}>
      {days.map(day => {
        const key = dateKey(day);
        const dayEvents = eventsByDay.get(key) ?? [];
        const outside = day.getMonth() !== month.getMonth();
        const shownEvents = dayEvents.slice(0, maxVisibleEvents);
        const remaining = dayEvents.length - shownEvents.length;
        const accessibleLabel = `${new Intl.DateTimeFormat('pt-BR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }).format(day)}, ${dayEvents.length} ${dayEvents.length === 1 ? 'evento' : 'eventos'}`;
        return <div key={key} role="gridcell" aria-selected={key === selectedKey} className={`day-cell ${outside ? 'outside' : ''} ${key === todayKey ? 'today' : ''} ${key === selectedKey ? 'selected' : ''}`} onClick={() => onDay(day)} onDoubleClick={() => { if (dayEvents.length === 0) onCreate(day); }}>
          <button className="day-number" data-date={key} tabIndex={key === selectedKey ? 0 : -1} aria-label={accessibleLabel} onClick={e => { e.stopPropagation(); onDay(day); }}>{day.getDate()}</button>
          <div className="day-events">
            {shownEvents.map(event => {
              const color = resolveEventColor(event, settings);
              const longTitle = event.summary || '(Sem título)';
              return <button key={`${event.calendarId}:${event.id}`} className="event-chip" style={{ '--event-color': color } as CSSProperties} onClick={e => { e.stopPropagation(); onEvent(event); }} title={longTitle}>
                <span className="event-color" />
                {event.start.dateTime && <small>{eventDisplayTime(event, settings.timeFormat)}</small>}
                <span>{longTitle}</span>
              </button>;
            })}
            {remaining > 0 && <button className="more-events" onClick={e => { e.stopPropagation(); onDay(day); }} aria-label={`Mostrar mais ${remaining} eventos`}>+{remaining} mais</button>}
          </div>
        </div>;
      })}
    </div>
  </section>;
}
