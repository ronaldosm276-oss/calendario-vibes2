import { useEffect, useState } from 'react';
import type { CalendarEvent, CalendarInfo, EventDraft, ReminderRule, Settings, SoundInfo } from '../types/calendar';
import { Bell, ChevronDown, Plus, Trash2, X } from 'lucide-react';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { normalizeHex } from '../utils/colors';
import { api } from '../services/tauriApi';

interface Props {
  event?: CalendarEvent;
  date: Date;
  calendars: CalendarInfo[];
  defaultCalendarId?: string;
  settings: Settings;
  onClose: () => void;
  onSave: (event: CalendarEvent, color: string | null, reminders: ReminderRule[]) => Promise<void>;
  eventColor?: string;
  colorFor: (calendarId: string) => string;
  onDelete?: () => Promise<void>;
}

function pad(value: number) { return String(value).padStart(2, '0'); }
function localDateString(date: Date) { return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`; }
function addOneDay(value: string) { const date = new Date(`${value}T00:00:00`); date.setDate(date.getDate() + 1); return localDateString(date); }
function normalizeEndDate(start: string, end: string) { return end > start ? end : addOneDay(start); }
function canWriteCalendar(calendar: CalendarInfo) { return !calendar.accessRole || ['owner', 'writer', 'writerWithoutPrivateAccess'].includes(calendar.accessRole); }

function draftFromEvent(event: CalendarEvent | undefined, date: Date, calendarId: string): EventDraft {
  if (!event) {
    const value = localDateString(date);
    return { calendarId, title: '', date: value, endDate: value, startTime: '09:00', endTime: '10:00', allDay: false, location: '', description: '' };
  }
  if (event.start.date) {
    return { calendarId: event.calendarId, title: event.summary, date: event.start.date, endDate: event.end.date || addOneDay(event.start.date), startTime: '09:00', endTime: '10:00', allDay: true, location: event.location || '', description: event.description || '' };
  }
  const start = event.start.dateTime ? new Date(event.start.dateTime) : date;
  const end = event.end.dateTime ? new Date(event.end.dateTime) : new Date(start.getTime() + 3600000);
  return { calendarId: event.calendarId, title: event.summary, date: localDateString(start), endDate: localDateString(end), startTime: `${pad(start.getHours())}:${pad(start.getMinutes())}`, endTime: `${pad(end.getHours())}:${pad(end.getMinutes())}`, allDay: false, location: event.location || '', description: event.description || '' };
}

const reminderOptions = [5, 10, 15, 30, 60, 120, 1440];
function reminderLabel(minutes: number) {
  if (minutes < 60) return `${minutes} minuto${minutes === 1 ? '' : 's'} antes`;
  if (minutes % 1440 === 0) return `${minutes / 1440} dia${minutes === 1440 ? '' : 's'} antes`;
  const hours = minutes / 60;
  return `${hours} hora${hours === 1 ? '' : 's'} antes`;
}

export function EventDialog({ event, date, calendars, defaultCalendarId, settings, onClose, onSave, onDelete, eventColor, colorFor }: Props) {
  const writableCalendars = calendars.filter(canWriteCalendar);
  const currentEventCalendar = event ? calendars.find(c => c.id === event.calendarId) : undefined;
  const selectableCalendars = event ? (currentEventCalendar ? [currentEventCalendar] : writableCalendars.filter(c => c.id === event.calendarId)) : writableCalendars;
  const initialCalendarId = event?.calendarId || (defaultCalendarId && writableCalendars.some(c => c.id === defaultCalendarId) ? defaultCalendarId : writableCalendars[0]?.id || '');
  const readOnlyEventCalendar = Boolean(event && currentEventCalendar && !canWriteCalendar(currentEventCalendar));

  const [draft, setDraft] = useState<EventDraft>(() => draftFromEvent(event, date, initialCalendarId));
  const [saving, setSaving] = useState(false);
  const [validationError, setValidationError] = useState('');
  const [showDetails, setShowDetails] = useState(Boolean(event?.location || event?.description));
  const [color, setColor] = useState<string | null>(normalizeHex(eventColor));
  const [reminders, setReminders] = useState<ReminderRule[]>(() => event ? (settings.reminders[eventKey(event)] ?? []).map(rule => ({ ...rule })) : (settings.defaultReminderEnabled ? [{ minutesBefore: settings.defaultReminderMinutes, sound: settings.defaultReminderSound ?? null, enabled: true }] : []));
  const [sounds, setSounds] = useState<SoundInfo[]>([]);
  const trapRef = useFocusTrap<HTMLDivElement>();

  useEffect(() => {
    setDraft(draftFromEvent(event, date, event?.calendarId || initialCalendarId));
    setValidationError('');
    setShowDetails(Boolean(event?.location || event?.description));
    setColor(normalizeHex(eventColor));
    setReminders(event ? (settings.reminders[eventKey(event)] ?? []).map(rule => ({ ...rule })) : (settings.defaultReminderEnabled ? [{ minutesBefore: settings.defaultReminderMinutes, sound: settings.defaultReminderSound ?? null, enabled: true }] : []));
  }, [event, date, initialCalendarId, eventColor, settings.defaultReminderEnabled, settings.defaultReminderMinutes, settings.defaultReminderSound, settings.reminders]);

  useEffect(() => { void api.listSounds().then(setSounds).catch(() => setSounds([])); }, []);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => { if (e.key === 'Escape' && !saving) onClose(); };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [onClose, saving]);

  const addReminder = () => setReminders(current => [...current, { minutesBefore: settings.defaultReminderMinutes || 10, sound: settings.defaultReminderSound ?? null, enabled: true }]);
  const updateReminder = (index: number, patch: Partial<ReminderRule>) => setReminders(current => current.map((rule, i) => i === index ? { ...rule, ...patch } : rule));
  const removeReminder = (index: number) => setReminders(current => current.filter((_, i) => i !== index));

  const save = async () => {
    setValidationError('');
    if (!draft.title.trim()) { setValidationError('Informe um título.'); return; }
    if (!draft.calendarId) { setValidationError('Escolha um calendário.'); return; }
    if (!draft.allDay) {
      const startLocal = new Date(`${draft.date}T${draft.startTime}:00`);
      const endLocal = new Date(`${draft.endDate}T${draft.endTime}:00`);
      if (!(endLocal > startLocal)) { setValidationError('O fim precisa ser depois do início.'); return; }
    }
    setSaving(true);
    try {
      const start = draft.allDay ? { date: draft.date } : { dateTime: `${draft.date}T${draft.startTime}:00`, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone };
      const end = draft.allDay ? { date: normalizeEndDate(draft.date, draft.endDate) } : { dateTime: `${draft.endDate}T${draft.endTime}:00`, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone };
      const payload: CalendarEvent = event
        ? { ...event, calendarId: draft.calendarId, summary: draft.title.trim(), description: draft.description, location: draft.location, start, end }
        : { id: '', calendarId: draft.calendarId, summary: draft.title.trim(), description: draft.description, location: draft.location, start, end };
      await onSave(payload, color, reminders.filter(rule => rule.enabled && rule.minutesBefore >= 0));
      onClose();
    } finally { setSaving(false); }
  };

  return <div className="modal-backdrop" role="presentation" onMouseDown={e => { if (e.currentTarget === e.target && !saving) onClose(); }}>
    <div className="modal event-dialog" ref={trapRef} role="dialog" aria-modal="true" aria-label={event ? 'Editar evento' : 'Novo evento'}>
      <div className="modal-header">
        <div><strong>{event ? 'Editar evento' : 'Novo evento'}</strong><span>{event ? 'Alterações serão sincronizadas com o Google Agenda.' : 'Crie um evento no calendário selecionado.'}</span></div>
        <button aria-label="Fechar" onClick={onClose}><X size={17} /></button>
      </div>

      <label className="title-field">Título<input autoFocus maxLength={500} value={draft.title} onChange={e => setDraft({ ...draft, title: e.target.value })} placeholder="Nome do evento" /></label>

      <div className="event-schedule-card">
        {draft.allDay ? (
          <div className="form-row"><label>Início<input type="date" value={draft.date} onChange={e => setDraft({ ...draft, date: e.target.value, endDate: normalizeEndDate(e.target.value, draft.endDate) })} /></label><label>Fim<input type="date" value={draft.endDate} min={addOneDay(draft.date)} onChange={e => setDraft({ ...draft, endDate: e.target.value })} /></label></div>
        ) : (
          <div className="form-row"><label>Início<input type="date" value={draft.date} onChange={e => setDraft({ ...draft, date: e.target.value, endDate: draft.endDate < e.target.value ? e.target.value : draft.endDate })} /><input type="time" value={draft.startTime} onChange={e => setDraft({ ...draft, startTime: e.target.value })} aria-label="Horário de início" /></label><label>Fim<input type="date" value={draft.endDate} min={draft.date} onChange={e => setDraft({ ...draft, endDate: e.target.value })} /><input type="time" value={draft.endTime} onChange={e => setDraft({ ...draft, endTime: e.target.value })} aria-label="Horário de fim" /></label></div>
        )}
        <label className="switch-row"><input type="checkbox" checked={draft.allDay} onChange={e => setDraft({ ...draft, allDay: e.target.checked })} /><span>Dia inteiro</span></label>
      </div>

      <label>Calendário<select disabled={Boolean(event) || readOnlyEventCalendar} value={draft.calendarId} onChange={e => setDraft({ ...draft, calendarId: e.target.value })}>{selectableCalendars.map(c => <option key={c.id} value={c.id}>{c.summary}</option>)}</select></label>
      {readOnlyEventCalendar && <div className="form-error" role="alert">Este calendário é somente leitura.</div>}
      {validationError && <div className="form-error" role="alert">{validationError}</div>}

      <section className="reminder-section">
        <div className="section-line"><div className="section-title"><Bell size={14} /> Lembretes</div><button type="button" className="icon-text-button" onClick={addReminder}><Plus size={14} /> Adicionar</button></div>
        {reminders.length === 0 ? <div className="setting-hint reminder-empty">Nenhum lembrete para este evento.</div> : reminders.map((rule, index) => <div className="reminder-row" key={`${index}-${rule.minutesBefore}-${rule.sound ?? ''}`}>
          <select value={rule.minutesBefore} onChange={e => updateReminder(index, { minutesBefore: Number(e.target.value) })}>{reminderOptions.map(value => <option key={value} value={value}>{reminderLabel(value)}</option>)}</select>
          <select value={rule.sound ?? ''} onChange={e => updateReminder(index, { sound: e.target.value || null })}>
            <option value="">Som padrão</option>
            {sounds.map(sound => <option key={sound.name} value={sound.name}>{sound.name}</option>)}
          </select>
          <button type="button" className="icon-button" aria-label={`Excluir lembrete ${index + 1}`} onClick={() => removeReminder(index)}><Trash2 size={14} /></button>
        </div>)}
      </section>

      <button type="button" className="details-toggle" onClick={() => setShowDetails(v => !v)} aria-expanded={showDetails}><span>{showDetails ? 'Ocultar detalhes' : 'Mais detalhes'}</span><ChevronDown size={15} className={showDetails ? 'chevron open' : 'chevron'} /></button>
      {showDetails && <div className="event-details">
        <label>Local<input maxLength={1000} value={draft.location} onChange={e => setDraft({ ...draft, location: e.target.value })} placeholder="Adicionar local" /></label>
        <label>Cor<div className="color-row"><input type="color" value={color ?? colorFor(draft.calendarId)} onChange={e => setColor(e.target.value)} />{color && <button type="button" className="reset-btn" onClick={() => setColor(null)}>Usar cor do calendário</button>}</div></label>
        <label>Descrição<textarea maxLength={20000} rows={4} value={draft.description} onChange={e => setDraft({ ...draft, description: e.target.value })} placeholder="Adicionar uma descrição" /></label>
      </div>}

      <div className="modal-actions">{event && onDelete && <button className="danger-button" disabled={saving} onClick={() => void onDelete()}>Excluir</button>}<span /><button onClick={onClose} disabled={saving}>Cancelar</button><button className="primary-button" disabled={saving || readOnlyEventCalendar || !draft.title.trim() || !draft.calendarId} onClick={() => void save()}>{saving ? 'Salvando…' : 'Salvar'}</button></div>
    </div>
  </div>;
}

function eventKey(event: Pick<CalendarEvent, 'calendarId' | 'id'>) { return `${event.calendarId}:${event.id}`; }
