import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useTasksStore } from '../../stores/tasks';

describe('lastError retriedTimes preservation', () => {
  beforeEach(()=>{ setActivePinia(createPinia()); });
  it('preserves previous retriedTimes when next code event lacks retriedTimes', () => {
    const store = useTasksStore();
    store.setLastError('A', { category:'Protocol', message:'retry attempt err', retriedTimes:2 });
    expect(store.lastErrorById['A'].retriedTimes).toBe(2);
    // informational event without retriedTimes
    store.setLastError('A', { category:'Protocol', message:'http override applied: follow=false max=0', code:'http_strategy_override_applied' });
    expect(store.lastErrorById['A'].retriedTimes).toBe(2);
  });

  it('updates retriedTimes when new event has higher retriedTimes', () => {
    const store = useTasksStore();
    store.setLastError('B', { category:'Protocol', message:'attempt1', retriedTimes:1 });
    expect(store.lastErrorById['B'].retriedTimes).toBe(1);
    store.setLastError('B', { category:'Protocol', message:'attempt2', retriedTimes:2 });
    expect(store.lastErrorById['B'].retriedTimes).toBe(2);
    // informational override without retriedTimes keeps 2
    store.setLastError('B', { category:'Protocol', code:'retry_strategy_override_applied', message:'retry override applied: max=2 baseMs=300 factor=1.5 jitter=true' });
    expect(store.lastErrorById['B'].retriedTimes).toBe(2);
  });
});
