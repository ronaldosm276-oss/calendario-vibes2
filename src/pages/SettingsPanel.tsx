import { useEffect, useState } from 'react';
import type { AuthState, CalendarInfo, Settings, SoundInfo } from '../types/calendar';
import { FolderOpen, Play, Plus, RotateCcw, Trash2, Volume2, X } from 'lucide-react';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { DEFAULT_EVENT_COLOR, normalizeHex } from '../utils/colors';
import { api } from '../services/tauriApi';
import { enable as enableAutostart, disable as disableAutostart } from '@tauri-apps/plugin-autostart';
import { open } from '@tauri-apps/plugin-dialog';

interface Props {
  settings: Settings;
  calendars: CalendarInfo[];
  auth: AuthState;
  onChange: (s: Settings) => void;
  onClose: () => void;
  onSignIn: () => void;
  onSignOut: () => void;
  onError: (message: string) => void;
}

export function SettingsPanel({ settings, calendars, auth, onChange, onClose, onSignIn, onSignOut, onError }: Props) {
  const trapRef = useFocusTrap<HTMLElement>(onClose);
  const [sounds, setSounds] = useState<SoundInfo[]>([]);
  const [soundBusy, setSoundBusy] = useState(false);

  const loadSounds = () => void api.listSounds().then(setSounds).catch(error => onError(error instanceof Error ? error.message : String(error)));
  useEffect(loadSounds, []);

  const patch = async (p: Partial<Settings>) => {
    if ('startWithWindows' in p) {
      try { if (p.startWithWindows) await enableAutostart(); else await disableAutostart(); }
      catch (error) { onError(error instanceof Error ? error.message : String(error)); return; }
    }
    onChange({ ...settings, ...p });
  };

  const toggleCalendar = (id: string) => patch({ visibleCalendarIds: settings.visibleCalendarIds.includes(id) ? settings.visibleCalendarIds.filter(x => x !== id) : [...settings.visibleCalendarIds, id] });

  const addSound = async () => {
    if (soundBusy) return;
    try {
      const selected = await open({ multiple: false, directory: false, filters: [{ name: 'Áudio', extensions: ['wav', 'mp3'] }] });
      const path = typeof selected === 'string' ? selected : null;
      if (!path) return;
      setSoundBusy(true);
      await api.addSound(path);
      loadSounds();
    } catch (error) { onError(error instanceof Error ? error.message : String(error)); }
    finally { setSoundBusy(false); }
  };

  const deleteSound = async (name: string) => {
    if (!window.confirm(`Excluir o som “${name}” desta aplicação?`)) return;
    try {
      await api.removeSound(name);
      const reminders = Object.fromEntries(Object.entries(settings.reminders ?? {}).map(([key, rules]) => [key, rules.map(rule => rule.sound === name ? { ...rule, sound: null } : rule)]));
      onChange({ ...settings, defaultReminderSound: settings.defaultReminderSound === name ? null : settings.defaultReminderSound, reminders });
      loadSounds();
    }
    catch (error) { onError(error instanceof Error ? error.message : String(error)); }
  };

  const testSound = async (sound: string | null = settings.defaultReminderSound ?? null) => {
    try { await api.playSound(sound, settings.alarmVolume, false); }
    catch (error) { onError(error instanceof Error ? error.message : String(error)); }
  };

  return <div className="settings-backdrop" role="presentation" onMouseDown={e => { if (e.currentTarget === e.target) onClose(); }}>
    <aside className="settings-panel" ref={trapRef} role="dialog" aria-modal="true" aria-label="Configurações">
      <header className="settings-header"><div><strong>Configurações</strong><span>Calendário</span></div><button className="settings-close" aria-label="Fechar configurações" onClick={onClose}><X size={17} /></button></header>
      <div className="settings-scroll">
        <section>
          <h3>Aparência</h3>
          <label>Tema<select value={settings.theme} onChange={e => patch({theme: e.target.value as Settings['theme']})}><option value="system">Sistema</option><option value="light">Claro</option><option value="dark">Escuro</option><option value="contrast">Alto contraste</option></select></label>
          <label>Cor de destaque<div className="color-row"><input type="color" value={normalizeHex(settings.accentColor) || '#2864d7'} onChange={e => patch({accentColor: e.target.value})}/>{settings.accentColor && <button className="reset-btn" onClick={() => patch({accentColor: undefined})}><RotateCcw size={12} /> Restaurar</button>}</div></label>
          <label>Escala do calendário: {Math.round((settings.zoom ?? 1) * 100)}%<input type="range" min="0.7" max="1.5" step="0.1" value={Math.min(1.5, settings.zoom ?? 1)} onChange={e => patch({zoom: Number(e.target.value)})}/><span className="setting-hint">Afeta somente o calendário e o painel do dia. As configurações permanecem em tamanho estável.</span></label>
        </section>

        <section>
          <h3>Janela</h3>
          <Check label="Sempre no topo" checked={settings.alwaysOnTop} onChange={v => patch({alwaysOnTop:v})}/>
          <Check label="Iniciar com o Windows" checked={settings.startWithWindows} onChange={v => patch({startWithWindows:v})}/>
          <Check label="Iniciar oculto" checked={settings.startHidden} onChange={v => patch({startHidden:v})}/>
          <Check label="Lembrar posição" checked={settings.rememberPosition} onChange={v => patch({rememberPosition:v})}/>
          <Check label="Lembrar tamanho" checked={settings.rememberSize} onChange={v => patch({rememberSize:v})}/>
          <label>Opacidade<input type="range" min="0.75" max="1" step="0.01" value={settings.opacity} onChange={e => patch({opacity:Number(e.target.value)})}/></label>
        </section>

        <section>
          <h3>Calendário</h3>
          <label>Calendário padrão<select value={settings.defaultCalendarId || ''} onChange={e => patch({defaultCalendarId:e.target.value})}>{calendars.map(c => <option key={c.id} value={c.id}>{c.summary}</option>)}</select></label>
          <label>A semana começa em<select value={settings.firstDayOfWeek} onChange={e => patch({firstDayOfWeek:Number(e.target.value) as 0|1})}><option value="1">Segunda-feira</option><option value="0">Domingo</option></select></label>
          <label>Formato de hora<select value={settings.timeFormat} onChange={e => patch({timeFormat:e.target.value as Settings['timeFormat']})}><option value="24h">24 horas</option><option value="12h">12 horas</option></select></label>
          <div className="settings-subheading">Calendários visíveis</div>
          <div className="calendar-checks">{calendars.map(c => { const local = normalizeHex(settings.calendarColors?.[c.id]); const shown = local || normalizeHex(c.backgroundColor) || DEFAULT_EVENT_COLOR; return <div className="cal-row" key={c.id}><Check label={c.summary} checked={settings.visibleCalendarIds.includes(c.id)} onChange={() => toggleCalendar(c.id)} color={shown}/><input type="color" aria-label={`Cor de ${c.summary}`} value={shown} onChange={e => patch({calendarColors: {...(settings.calendarColors ?? {}), [c.id]: e.target.value}})}/>{local && <button className="reset-btn" onClick={() => { const next = {...(settings.calendarColors ?? {})}; delete next[c.id]; patch({calendarColors: next}); }}><RotateCcw size={12} /></button>}</div>; })}</div>
        </section>

        <section>
          <h3>Lembretes</h3>
          <Check label="Ativar lembretes" checked={settings.remindersEnabled} onChange={v => patch({remindersEnabled:v})}/>
          <label>Lembrete padrão<select disabled={!settings.defaultReminderEnabled} value={settings.defaultReminderEnabled ? settings.defaultReminderMinutes : 0} onChange={e => patch({defaultReminderEnabled: true, defaultReminderMinutes:Number(e.target.value)})}><option value="5">5 minutos antes</option><option value="10">10 minutos antes</option><option value="15">15 minutos antes</option><option value="30">30 minutos antes</option><option value="60">1 hora antes</option><option value="120">2 horas antes</option><option value="1440">1 dia antes</option></select></label>
          <Check label="Adicionar o lembrete padrão a novos eventos" checked={settings.defaultReminderEnabled} onChange={v => patch({defaultReminderEnabled:v})}/>
          <label>Som padrão<select value={settings.defaultReminderSound ?? ''} onChange={e => patch({defaultReminderSound:e.target.value || null})}><option value="">Som padrão do alarme</option>{sounds.map(sound => <option key={sound.name} value={sound.name}>{sound.name}</option>)}</select></label>
          <label>Volume: {Math.round(settings.alarmVolume * 100)}%<input type="range" min="0" max="1" step="0.05" value={settings.alarmVolume} onChange={e => patch({alarmVolume:Number(e.target.value)})}/></label>
          <label>Soneca<select value={settings.snoozeMinutes} onChange={e => patch({snoozeMinutes:Number(e.target.value)})}><option value="1">1 minuto</option><option value="5">5 minutos</option><option value="10">10 minutos</option><option value="15">15 minutos</option><option value="30">30 minutos</option></select></label>
          <button className="secondary-button full-button" onClick={() => void testSound()}><Volume2 size={14} /> Testar som</button>

          <div className="settings-subheading">Sons personalizados</div>
          <div className="sound-actions"><button className="secondary-button" onClick={() => void addSound()} disabled={soundBusy}><Plus size={14} /> Adicionar som</button><button className="secondary-button" onClick={() => void api.openSoundsFolder()}><FolderOpen size={14} /> Abrir pasta</button></div>
          <span className="setting-hint">Coloque arquivos WAV ou MP3 na pasta de sons para usá-los nos lembretes.</span>
          <div className="sounds-list">
            {sounds.length === 0 ? <div className="muted">Nenhum som personalizado.</div> : sounds.map(sound => <div className="sound-row" key={sound.name}><span title={sound.name}>{sound.name}</span><button aria-label={`Testar ${sound.name}`} onClick={() => void testSound(sound.name)}><Play size={13}/></button><button aria-label={`Excluir ${sound.name}`} onClick={() => void deleteSound(sound.name)}><Trash2 size={13}/></button></div>)}
          </div>
        </section>

        <section>
          <h3>Sincronização</h3>
          <Check label="Sincronização automática" checked={settings.automaticSync} onChange={v => patch({automaticSync:v})}/>
          <label>Intervalo<select value={settings.syncIntervalMinutes} onChange={e => patch({syncIntervalMinutes:Number(e.target.value)})}><option value="5">5 minutos</option><option value="15">15 minutos</option><option value="30">30 minutos</option><option value="60">1 hora</option></select></label>
        </section>

        <section>
          <h3>Conta</h3>
          {auth.authenticated ? <><div className="account-card"><div className="avatar">{auth.picture ? <img src={auth.picture} alt="" /> : (auth.name?.slice(0,1) || '?')}</div><div><strong>{auth.name || 'Conta Google'}</strong><small>{auth.email || 'Conta Google conectada'}</small></div></div><button className="danger-button full" onClick={onSignOut}>Sair da conta</button></> : <button className="primary-button full" onClick={onSignIn}>Entrar com Google</button>}
        </section>

        <section className="about-section"><h3>Sobre</h3><p className="muted">Calendário · 0.1.0</p></section>
      </div>
    </aside>
  </div>;
}

function Check({label, checked, onChange, color}: {label:string;checked:boolean;onChange:(v:boolean)=>void;color?:string}) {
  return <label className="check-row"><input type="checkbox" checked={checked} onChange={e=>onChange(e.target.checked)}/>{color && <span className="color-dot" style={{background:color}}/>}<span>{label}</span></label>;
}
