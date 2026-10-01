import type { CalendarEvent, CalendarInfo, Settings } from '../types/calendar';
import { Bell, CalendarPlus, X } from 'lucide-react';
import { eventDisplayTime, formatFullDate } from '../utils/date';
import { resolveEventColor } from '../utils/colors';
import type React from 'react';

interface Props {
  date: Date;
  relativeText: string;
  events: CalendarEvent[];
  calendars: CalendarInfo[];
  settings: Settings;
  onClose: () => void;
  onEvent: (event: CalendarEvent) => void;
  onCreate: () => void;
}

export function SelectedDayPopover({ date, relativeText, events, calendars, settings, onClose, onEvent, onCreate }: Props) {
  return (
    <div className="day-popover-backdrop" role="presentation" onMouseDown={e => { if (e.currentTarget === e.target) onClose(); }}>
      <section className="day-popover" role="dialog" aria-modal="false" aria-label={formatFullDate(date)}>
        <header className="day-popover-header">
          <div className="day-popover-heading">
            <strong>{formatFullDate(date)}</strong>
            <span>{relativeText}</span>
          </div>
          <button className="settings-close" aria-label="Fechar" onClick={onClose}><X size={17} /></button>
        </header>

        <div className="day-popover-content">
          {events.length === 0 ? (
            <button className="popover-empty" onClick={onCreate}>
              <span>Nenhum evento neste dia.</span>
              <strong><CalendarPlus size={14} /> Criar evento</strong>
            </button>
          ) : events.map(event => {
            const calendar = calendars.find(c => c.id === event.calendarId);
            const color = resolveEventColor(event, settings);
            return (
              <button key={`${event.calendarId}:${event.id}`} className="popover-event" style={{ '--event-color': color } as React.CSSProperties} onClick={() => onEvent(event)}>
                <span className="popover-event-color" />
                <span className="popover-event-time">{event.start.date ? 'Dia inteiro' : eventDisplayTime(event, settings.timeFormat)}</span>
                <span className="popover-event-copy">
                  <strong>{event.summary || '(Sem título)'}</strong>
                  <small>{calendar?.summary || ''}{event.location ? ` · ${event.location}` : ''}</small>
                </span>
                {settings.remindersEnabled && (settings.reminders[`${event.calendarId}:${event.id}`]?.length ?? 0) > 0 && <Bell size={13} className="popover-reminder-icon" aria-label="Com lembrete" />}
              </button>
            );
          })}
        </div>

        <footer className="day-popover-footer">
          <button className="popover-new-button" onClick={onCreate}><CalendarPlus size={14} /> Novo evento</button>
        </footer>
      </section>
    </div>
  );
}
