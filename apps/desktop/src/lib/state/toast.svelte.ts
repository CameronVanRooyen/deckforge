export interface Toast {
  id: number;
  kind: 'info' | 'ok' | 'error';
  text: string;
}

class ToastStore {
  items = $state<Toast[]>([]);
  private n = 0;

  push(kind: Toast['kind'], text: string, ms = 4000) {
    const id = ++this.n;
    this.items = [...this.items.slice(-3), { id, kind, text }];
    setTimeout(() => this.dismiss(id), ms);
  }
  info = (t: string) => this.push('info', t);
  ok = (t: string) => this.push('ok', t);
  error = (t: string) => this.push('error', t, 6000);
  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
