import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const listenMock = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a:any[]) => listenMock(...a) }));
import { initTaskEvents, disposeTaskEvents } from '../tasks';
import { useTasksStore } from '../../stores/tasks';

interface L { evt:string; cb:(p:any)=>void }

describe('multiple applied override events update store each time', () => {
  beforeEach(()=>{
    setActivePinia(createPinia());
    listenMock.mockReset();
    (listenMock as any)._calls=[];
    listenMock.mockImplementation((evt:string, cb:any)=>{ const l:L={evt,cb}; (listenMock as any)._calls.push(l); return Promise.resolve(()=>{l.cb=()=>{}}); });
  });

  it('http -> tls -> retry updates code sequentially', async () => {
    await initTaskEvents();
    const store = useTasksStore();
    const errL = ((listenMock as any)._calls as L[]).find(c=>c.evt==='task://error')!;
    errL.cb({ payload: { taskId:'Z', kind:'GitFetch', category:'Protocol', code:'http_strategy_override_applied', message:'http override applied: follow=false max=2' } });
    expect(store.lastErrorById['Z'].code).toBe('http_strategy_override_applied');
    errL.cb({ payload: { taskId:'Z', kind:'GitFetch', category:'Protocol', code:'tls_strategy_override_applied', message:'tls override applied: insecureSkipVerify=true skipSanWhitelist=false' } });
    expect(store.lastErrorById['Z'].code).toBe('tls_strategy_override_applied');
    errL.cb({ payload: { taskId:'Z', kind:'GitFetch', category:'Protocol', code:'retry_strategy_override_applied', message:'retry override applied: max=2 baseMs=300 factor=1.5 jitter=true' } });
    expect(store.lastErrorById['Z'].code).toBe('retry_strategy_override_applied');
    disposeTaskEvents();
  });
});
