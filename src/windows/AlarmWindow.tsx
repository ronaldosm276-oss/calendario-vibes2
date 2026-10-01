import { useEffect, useMemo, useState } from 'react';
import { AlarmClock, BellRing, Check, ExternalLink, Moon, Volume2 } from 'lucide-react';
import { emit, listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { api } from '../services/tauriApi';
import type { AlarmInfo, Settings } from '../types/calendar';

const defaultSettings: Pick<Settings, 'alarmVolume' | 'snoozeMinutes'> = { alarmVolume: 0.9, snoozeMinutes: 5 };

function formatAlarmDate(value: string) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return value;
  return new Intl.DateTimeFormat('pt-BR', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }).format(date);
}

export function AlarmWindow() {
  const [alarms, setAlarms] = useState<AlarmInfo[]>([]);
  const [settings, setSettings] = useState(defaultSettings);
  const [busy, setBusy] = useState(false);

  const current = alarms[0];
  const extraCount = Math.max(0, alarms.length - 1);
  const startLabel = useMemo(() => current ? formatAlarmDate(current.startAt) : '', [current]);

  const refresh = async () => {
    const [active, saved] = await Promise.all([api.getActiveAlarms(), api.getSettings()]);
    setAlarms(active);
    setSettings({ alarmVolume: saved.alarmVolume ?? 0.9, snoozeMinutes: saved.snoozeMinutes ?? 5 });
  };

  useEffect(() => {
    void refresh().catch(console.error);
    let off: (() => void) | undefined;
    void listen<AlarmInfo>('reminder-triggered', () => { void refresh().catch(console.error); }).then(unlisten => { off = unlisten; });
    return () => off?.();
  }, []);

  useEffect(() => {
    if (current) void api.playSound(current.sound ?? null, settings.alarmVolume, true).catch(console.error);
    else void api.stopSound().catch(console.error);
  }, [current?.id, current?.sound, settings.alarmVolume]);

  const hide = async () => {
    const window = await WebviewWindow.getByLabel('alarm');
    await window?.hide().catch(() => undefined);
  };

  const acknowledge = async () => {
    if (!current || busy) return;
    setBusy(true);
    try { await api.acknowledgeAlarm(current.id); await refresh(); if (alarms.length <= 1) await hide(); }
    finally { setBusy(false); }
  };

  const snooze = async () => {
    if (!current || busy) return;
    setBusy(true);
    try { await api.snoozeAlarm(current.id, settings.snoozeMinutes); await refresh(); if (alarms.length <= 1) await hide(); }
    finally { setBusy(false); }
  };

  const openEvent = async () => {
    if (!current || busy) return;
    setBusy(true);
    try {
      await emit('alarm-open-event', { eventKey: current.eventKey, startAt: current.startAt });
      await api.acknowledgeAlarm(current.id);
      await hide();
    } finally { setBusy(false); }
  };

  if (!current) {
    return <div className="alarm-window idle"><span>Nenhum alarme ativo.</span></div>;
  }

  return <div className="alarm-window" role="dialog" aria-modal="true" aria-label="Lembrete">
    <div className="alarm-accent" />
    <header className="alarm-header"><div className="alarm-icon"><BellRing size={21} /></div><div><strong>Lembrete</strong><span>Hora do evento</span></div></header>
    <main className="alarm-body">
      <h1>{current.title || 'Evento sem título'}</h1>
      <div className="alarm-time"><AlarmClock size={16} /><span>{startLabel}</span></div>
      <div className="alarm-audio"><Volume2 size={14} /><span>{current.sound ? `Som: ${current.sound}` : 'Som padrão do alarme'}</span></div>
      {extraCount > 0 && <div className="alarm-queue">+{extraCount} outro{extraCount === 1 ? '' : 's'} lembrete{extraCount === 1 ? '' : 's'} aguardando</div>}
    </main>
    <footer className="alarm-actions">
      <button className="alarm-secondary" disabled={busy} onClick={() => void snooze()}><Moon size={15} /> Soneca {settings.snoozeMinutes} min</button>
      <button className="alarm-secondary" disabled={busy} onClick={() => void openEvent()}><ExternalLink size={15} /> Abrir evento</button>
      <button className="alarm-primary" disabled={busy} onClick={() => void acknowledge()}><Check size={15} /> Reconhecer</button>
    </footer>
  </div>;
}
