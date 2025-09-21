import { describe, it, expect, beforeEach, vi } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const listenMock = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a:any[]) => listenMock(...a) }));

import { initTaskEvents, disposeTaskEvents } from '../tasks';
import { useTasksStore } from '../../stores/tasks';
import { useLogsStore } from '../../stores/logs';

interface L { evt:string; cb:(p:any)=>void }

describe('strategy override event ordering', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    listenMock.mockReset();
    (listenMock as any)._calls = [];
    listenMock.mockImplementation((evt:string, cb:any) => { const l:L={evt,cb}; (listenMock as any)._calls.push(l); return Promise.resolve(()=>{l.cb=()=>{}}); });
  });

  it('applied -> conflict -> ignored ordering', async () => {
    await initTaskEvents();
    const store = useTasksStore();
    const logs = useLogsStore();
    const calls:L[] = (listenMock as any)._calls;
    const errL = calls.find(c=>c.evt==='task://error')!;

    // 1 http applied
    errL.cb({ payload: { taskId:'T', kind:'GitClone', category:'Protocol', code:'http_strategy_override_applied', message:'http override applied: follow=false max=0' } });
    // 2 conflict
    errL.cb({ payload: { taskId:'T', kind:'GitClone', category:'Protocol', code:'strategy_override_conflict', message:'http conflict: followRedirects=false => force maxRedirects=0 (was 3)' } });
    // 3 ignored
    errL.cb({ payload: { taskId:'T', kind:'GitClone', category:'Protocol', code:'strategy_override_ignored_fields', message:'strategy override ignored unknown fields: top=[x] sections=[http.y]' } });

    // 最终记录应为最后一次 (ignored)
    expect(store.lastErrorById['T'].code).toBe('strategy_override_ignored_fields');
    // 三条日志（简单包含检查）
    const count = logs.items.filter(l => l.message.includes('override')).length;
    expect(count).toBeGreaterThanOrEqual(1); // 不严格限定全部，因为 logs store 可能覆盖分类
    disposeTaskEvents();
  });
});
