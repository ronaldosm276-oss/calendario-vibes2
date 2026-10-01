import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props { children: ReactNode; }
interface State { error: Error | null; }

export class AppErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State { return { error }; }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Erro de renderização do Calendário', error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return <div style={{ minHeight:'100vh', boxSizing:'border-box', padding:20, background:'#1c1d22', color:'#f1f3f6', fontFamily:'system-ui, sans-serif' }}>
      <strong>O calendário não pôde ser carregado.</strong>
      <p style={{ color:'#a4a9b4', whiteSpace:'pre-wrap' }}>{this.state.error.message}</p>
    </div>;
  }
}
