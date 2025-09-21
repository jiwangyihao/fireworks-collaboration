import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

// mock listen to immediately invoke callbacks when we push events manually
const listenMock = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({ listen: (...args: any[]) => listenMock(...args) }));

import { initTaskEvents, disposeTaskEvents } from '../tasks';
import { useTasksStore } from '../../stores/tasks';

interface Listener { evt: string; cb: (e: any) => void }

describe('task error events propagate code', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    (listenMock as any)._calls = [];
    listenMock.mockImplementation((evt: string, cb: any) => {
      const l: Listener = { evt, cb };
      (listenMock as any)._calls.push(l);
      return Promise.resolve(() => { l.cb = () => {}; });
    });
  });

  it('stores code for strategy override applied', async () => {
    await initTaskEvents();
    const store = useTasksStore();
    const listeners: Listener[] = (listenMock as any)._calls;
    const errL = listeners.find(l => l.evt === 'task://error')!;
    errL.cb({ payload: { taskId: 'x1', kind: 'GitClone', category: 'Protocol', code: 'http_strategy_override_applied', message: 'http override applied: follow=false max=3' } });
    expect(store.lastErrorById['x1']).toMatchObject({ code: 'http_strategy_override_applied' });
    disposeTaskEvents();
  });
});
