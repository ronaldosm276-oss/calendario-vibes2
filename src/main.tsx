import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { AlarmWindow } from './windows/AlarmWindow';
import { AppErrorBoundary } from './AppErrorBoundary';
import './styles/app.css';

const isAlarmWindow = new URLSearchParams(window.location.search).get('janela') === 'alarme';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppErrorBoundary>
      {isAlarmWindow ? <AlarmWindow /> : <App />}
    </AppErrorBoundary>
  </React.StrictMode>
);
