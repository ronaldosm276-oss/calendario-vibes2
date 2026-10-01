import { CalendarDays, RefreshCw, Settings2 } from 'lucide-react';

interface Props { onSettings: () => void; onRefresh: () => void; syncing: boolean; syncLabel: string; }

export function TitleBar({ onSettings, onRefresh, syncing, syncLabel }: Props) {
  return (
    <div className="titlebar">
      <div className="brand" aria-hidden="true"><CalendarDays size={15} /></div>
      <div className="window-actions">
        <button title={`${syncLabel}. Atualizar`} aria-label={`Atualizar calendário. ${syncLabel}`} onClick={onRefresh} className={syncing ? 'spinning' : ''}><RefreshCw size={14} /></button>
        <button title="Configurações" aria-label="Configurações" onClick={onSettings}><Settings2 size={15} /></button>
      </div>
    </div>
  );
}
