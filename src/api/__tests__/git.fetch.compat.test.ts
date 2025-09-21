import { describe, it, expect, vi, beforeEach } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
import { startGitFetch } from '../tasks';

describe('startGitFetch backward compatibility', () => {
  beforeEach(()=>{ (invoke as any).mockReset(); });
  it('supports legacy preset string third argument', async () => {
    (invoke as any).mockResolvedValueOnce('legacy-id');
    const id = await startGitFetch('repo', 'C:/tmp/repo', 'branches+tags');
    expect(id).toBe('legacy-id');
    expect(invoke).toHaveBeenCalledWith('git_fetch', { repo:'repo', dest:'C:/tmp/repo', preset:'branches+tags' });
  });
});
