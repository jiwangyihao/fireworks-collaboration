import { describe, it, expect, beforeEach } from 'vitest';
import { setActivePinia, createPinia } from 'pinia';
import { useTasksStore } from '../../stores/tasks';

// We directly call setLastError to simulate backend emitted task://error events.
// This avoids needing tauri runtime in unit tests.

describe('strategy override informational events', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('records http/retry/tls applied event codes', () => {
    const store = useTasksStore();
    store.setLastError('t1', { category: 'Protocol', message: 'http override applied: follow=false max=3', code: 'http_strategy_override_applied' });
    store.setLastError('t2', { category: 'Protocol', message: 'retry override applied: max=2 baseMs=300 factor=1.5 jitter=true', code: 'retry_strategy_override_applied' });
    store.setLastError('t3', { category: 'Protocol', message: 'tls override applied: insecureSkipVerify=true skipSanWhitelist=false', code: 'tls_strategy_override_applied' });

    expect(store.lastErrorById['t1'].code).toBe('http_strategy_override_applied');
    expect(store.lastErrorById['t2'].code).toBe('retry_strategy_override_applied');
    expect(store.lastErrorById['t3'].code).toBe('tls_strategy_override_applied');
  });

  it('records conflict and ignored fields events', () => {
    const store = useTasksStore();
    store.setLastError('c1', { category: 'Protocol', message: 'http conflict: followRedirects=false => force maxRedirects=0 (was 2)', code: 'strategy_override_conflict' });
    store.setLastError('i1', { category: 'Protocol', message: 'strategy override ignored unknown fields: top=[x] sections=[http.y]', code: 'strategy_override_ignored_fields' });

    expect(store.lastErrorById['c1'].code).toBe('strategy_override_conflict');
    expect(store.lastErrorById['i1'].code).toBe('strategy_override_ignored_fields');
  });

  it('keeps retriedTimes compatibility when code present', () => {
    const store = useTasksStore();
    store.setLastError('r1', { category: 'Protocol', message: 'http override applied: follow=true max=5', code: 'http_strategy_override_applied', retriedTimes: 0 });
    expect(store.lastErrorById['r1'].retriedTimes).toBe(0);
  });

  it('records adaptive tls rollout event code', () => {
    const store = useTasksStore();
    store.setLastError('a1', { category: 'Protocol', message: '{"percentApplied":50,"sampled":true}', code: 'adaptive_tls_rollout' });
    expect(store.lastErrorById['a1'].code).toBe('adaptive_tls_rollout');
  });
});
