import { describe, it, expect, vi, beforeEach } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
import { startGitFetch } from '../tasks';

describe('startGitFetch legacy remote preset omission', () => {
  beforeEach(()=>{ (invoke as any).mockReset(); });
  it('does not send preset when preset == remote (legacy signature)', async () => {
    (invoke as any).mockResolvedValueOnce('id-remote');
    const id = await startGitFetch('repo','C:/tmp/x','remote');
    expect(id).toBe('id-remote');
    expect(invoke).toHaveBeenCalledWith('git_fetch', { repo:'repo', dest:'C:/tmp/x' });
  });
  it('does not send preset when preset == remote (object options)', async () => {
    (invoke as any).mockResolvedValueOnce('id-remote2');
    const id = await startGitFetch('repo2','C:/tmp/y',{ preset:'remote' });
    expect(id).toBe('id-remote2');
    expect(invoke).toHaveBeenCalledWith('git_fetch', { repo:'repo2', dest:'C:/tmp/y' });
  });
});
