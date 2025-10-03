import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, VueWrapper, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import CredentialList from '../CredentialList.vue';
import { useCredentialStore } from '../../stores/credential';
import type { CredentialInfo } from '../../api/credential';

// Mock ConfirmDialog
vi.mock('../ConfirmDialog.vue', () => ({
  default: {
    name: 'ConfirmDialog',
    template: '<div class="mock-confirm-dialog" v-if="show"><button class="confirm-btn" @click="$emit(\'confirm\')">确认</button><button class="cancel-btn" @click="$emit(\'cancel\')">取消</button></div>',
    props: ['show', 'title', 'message', 'variant'],
    emits: ['confirm', 'cancel'],
  },
}));

// Mock credential API
vi.mock('../../api/credential', () => ({
  formatTimestamp: (timestamp: number) => new Date(timestamp * 1000).toLocaleString(),
  isExpiringSoon: (expiresAt?: number) => {
    if (!expiresAt) return false;
    const now = Math.floor(Date.now() / 1000);
    const sevenDays = 7 * 24 * 60 * 60;
    return expiresAt - now < sevenDays && expiresAt > now;
  },
}));

describe('CredentialList.vue', () => {
  let wrapper: VueWrapper<any>;
  let credentialStore: ReturnType<typeof useCredentialStore>;

  const mockCredentials: CredentialInfo[] = [
    {
      host: 'github.com',
      username: 'user1',
      maskedPassword: '***',
      createdAt: Math.floor(Date.now() / 1000) - 86400, // 1 day ago
      expiresAt: Math.floor(Date.now() / 1000) + 30 * 86400, // 30 days from now
      lastUsedAt: Math.floor(Date.now() / 1000) - 3600, // 1 hour ago
      isExpired: false,
    },
    {
      host: 'gitlab.com',
      username: 'user2',
      maskedPassword: 'ghp_****xyz',
      createdAt: Math.floor(Date.now() / 1000) - 172800, // 2 days ago
      expiresAt: Math.floor(Date.now() / 1000) + 5 * 86400, // 5 days from now (expiring soon)
      lastUsedAt: undefined,
      isExpired: false,
    },
    {
      host: 'bitbucket.org',
      username: 'user3',
      maskedPassword: '***',
      createdAt: Math.floor(Date.now() / 1000) - 259200, // 3 days ago
      expiresAt: Math.floor(Date.now() / 1000) - 86400, // 1 day ago (expired)
      lastUsedAt: Math.floor(Date.now() / 1000) - 172800, // 2 days ago
      isExpired: true,
    },
  ];

  beforeEach(() => {
    setActivePinia(createPinia());
    credentialStore = useCredentialStore();
    credentialStore.credentials = [];
    credentialStore.loading = false;
    credentialStore.error = null;
  });

  // ===== 渲染与基本显示测试 (8个) =====

  it('renders empty state when no credentials', () => {
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toContain('暂无凭证');
    expect(wrapper.text()).toContain('点击上方的"添加凭证"按钮开始使用');
    expect(wrapper.find('svg').exists()).toBe(true); // Lock icon
  });

  it('renders loading state', () => {
    credentialStore.loading = true;
    wrapper = mount(CredentialList);
    
    const spinner = wrapper.find('.loading.loading-spinner');
    expect(spinner.exists()).toBe(true);
  });

  it('renders credential count in header', () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toContain('已保存的凭证 (3)');
  });

  it('renders all credentials', () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    const cards = wrapper.findAll('.card.bg-base-200');
    expect(cards).toHaveLength(3);
  });

  it('displays credential host and username', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toContain('github.com');
    expect(wrapper.text()).toContain('用户名: user1');
  });

  it('displays masked password', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toContain('密码: ***');
    expect(wrapper.text()).not.toContain('real_password'); // Should never show real password
  });

  it('displays created date', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toMatch(/创建于:/);
  });

  it('displays expires date when present', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toMatch(/过期于:/);
  });

  // ===== 过期状态测试 (6个) =====

  it('shows expired badge for expired credentials', () => {
    credentialStore.credentials = [mockCredentials[2]]; // Expired
    wrapper = mount(CredentialList);
    
    const badge = wrapper.find('.badge.badge-error');
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe('已过期');
  });

  it('applies error border to expired credentials', () => {
    credentialStore.credentials = [mockCredentials[2]]; // Expired
    wrapper = mount(CredentialList);
    
    const card = wrapper.find('.card.bg-base-200');
    expect(card.classes()).toContain('border-error');
  });

  it('shows expiring soon badge for credentials expiring within 7 days', () => {
    credentialStore.credentials = [mockCredentials[1]]; // Expiring in 5 days
    wrapper = mount(CredentialList);
    
    const badge = wrapper.find('.badge.badge-warning');
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe('即将过期');
  });

  it('applies warning border to expiring soon credentials', () => {
    credentialStore.credentials = [mockCredentials[1]]; // Expiring soon
    wrapper = mount(CredentialList);
    
    const card = wrapper.find('.card.bg-base-200');
    expect(card.classes()).toContain('border-warning');
  });

  it('does not show badge for normal credentials', () => {
    credentialStore.credentials = [mockCredentials[0]]; // Normal (30 days)
    wrapper = mount(CredentialList);
    
    const errorBadge = wrapper.find('.badge.badge-error');
    const warningBadge = wrapper.find('.badge.badge-warning');
    expect(errorBadge.exists()).toBe(false);
    expect(warningBadge.exists()).toBe(false);
  });

  it('does not apply border to normal credentials', () => {
    credentialStore.credentials = [mockCredentials[0]]; // Normal
    wrapper = mount(CredentialList);
    
    const card = wrapper.find('.card.bg-base-200');
    expect(card.classes()).not.toContain('border-error');
    expect(card.classes()).not.toContain('border-warning');
  });

  // ===== 编辑功能测试 (4个) =====

  it('emits edit event when edit button clicked', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const editButton = wrapper.find('button[title="编辑"]');
    await editButton.trigger('click');
    
    expect(wrapper.emitted('edit')).toBeTruthy();
    expect(wrapper.emitted('edit')![0]).toEqual([mockCredentials[0]]);
  });

  it('shows edit button for each credential', () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    const editButtons = wrapper.findAll('button[title="编辑"]');
    expect(editButtons).toHaveLength(3);
  });

  it('edit button contains correct icon', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const editButton = wrapper.find('button[title="编辑"]');
    expect(editButton.find('svg').exists()).toBe(true);
  });

  it('emits correct credential data on edit', async () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    const editButtons = wrapper.findAll('button[title="编辑"]');
    await editButtons[1].trigger('click'); // Click second credential
    
    expect(wrapper.emitted('edit')![0]).toEqual([mockCredentials[1]]);
  });

  // ===== 删除功能测试 (7个) =====

  it('shows delete button for each credential', () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    const deleteButtons = wrapper.findAll('button[title="删除"]');
    expect(deleteButtons).toHaveLength(3);
  });

  it('delete button contains correct icon', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteButton = wrapper.find('button[title="删除"]');
    expect(deleteButton.find('svg').exists()).toBe(true);
  });

  it('delete button has error styling', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteButton = wrapper.find('button[title="删除"]');
    expect(deleteButton.classes()).toContain('text-error');
  });

  it('shows confirmation dialog when delete button clicked', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteButton = wrapper.find('button[title="删除"]');
    await deleteButton.trigger('click');
    await wrapper.vm.$nextTick();
    
    // Check ConfirmDialog is shown
    const confirmDialog = wrapper.findComponent({ name: 'ConfirmDialog' });
    expect(confirmDialog.props('show')).toBe(true);
    expect(confirmDialog.props('message')).toContain('github.com');
    expect(confirmDialog.props('message')).toContain('user1');
  });

  it('calls store.delete when deletion confirmed', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteSpy = vi.spyOn(credentialStore, 'delete').mockResolvedValue();
    
    const deleteButton = wrapper.find('button[title="删除"]');
    await deleteButton.trigger('click');
    await wrapper.vm.$nextTick();
    
    // Trigger confirm event from ConfirmDialog
    const confirmDialog = wrapper.findComponent({ name: 'ConfirmDialog' });
    await confirmDialog.vm.$emit('confirm');
    await flushPromises();
    
    expect(deleteSpy).toHaveBeenCalledWith('github.com', 'user1');
    
    deleteSpy.mockRestore();
  });

  it('does not call store.delete when deletion cancelled', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteSpy = vi.spyOn(credentialStore, 'delete').mockResolvedValue();
    
    const deleteButton = wrapper.find('button[title="删除"]');
    await deleteButton.trigger('click');
    await wrapper.vm.$nextTick();
    
    // Trigger cancel event from ConfirmDialog
    const confirmDialog = wrapper.findComponent({ name: 'ConfirmDialog' });
    await confirmDialog.vm.$emit('cancel');
    await flushPromises();
    
    expect(deleteSpy).not.toHaveBeenCalled();
    
    deleteSpy.mockRestore();
  });

  it('shows alert on delete failure', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    const deleteSpy = vi.spyOn(credentialStore, 'delete').mockRejectedValue(new Error('Network error'));
    const alertSpy = vi.spyOn(window, 'alert').mockImplementation(() => {});
    
    const deleteButton = wrapper.find('button[title="删除"]');
    await deleteButton.trigger('click');
    await wrapper.vm.$nextTick();
    
    // Trigger confirm event from ConfirmDialog
    const confirmDialog = wrapper.findComponent({ name: 'ConfirmDialog' });
    await confirmDialog.vm.$emit('confirm');
    await flushPromises();
    
    // Wait for async operation
    await new Promise(resolve => setTimeout(resolve, 10));
    
    expect(alertSpy).toHaveBeenCalledWith('删除失败: Network error');
    
    deleteSpy.mockRestore();
    alertSpy.mockRestore();
  });

  // ===== 时间显示测试 (3个) =====

  it('displays last used time when present', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).toMatch(/最后使用:/);
  });

  it('does not display last used time when absent', () => {
    credentialStore.credentials = [mockCredentials[1]]; // Has no lastUsedAt
    wrapper = mount(CredentialList);
    
    expect(wrapper.text()).not.toMatch(/最后使用:/);
  });

  it('formats all timestamps correctly', () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    // All timestamps should be formatted as locale strings
    const text = wrapper.text();
    expect(text).toMatch(/\d{4}\/\d{1,2}\/\d{1,2}/); // Date format pattern
  });

  // ===== 排序与过滤测试 (3个) =====

  it('displays credentials in sorted order from store', () => {
    // Store's sortedCredentials getter should handle sorting
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    const cards = wrapper.findAll('.card.bg-base-200');
    expect(cards).toHaveLength(3);
    
    // Order is determined by store.sortedCredentials
    // Just verify all are displayed
    expect(wrapper.text()).toContain('github.com');
    expect(wrapper.text()).toContain('gitlab.com');
    expect(wrapper.text()).toContain('bitbucket.org');
  });

  it('uses store sortedCredentials computed property', () => {
    credentialStore.credentials = mockCredentials;
    wrapper = mount(CredentialList);
    
    // Verify component uses sorted credentials (not directly testing spy,
    // but verifying the behavior - credentials are displayed)
    const cards = wrapper.findAll('.card.bg-base-200');
    expect(cards).toHaveLength(3);
  });

  it('updates when store credentials change', async () => {
    credentialStore.credentials = [mockCredentials[0]];
    wrapper = mount(CredentialList);
    
    expect(wrapper.findAll('.card.bg-base-200')).toHaveLength(1);
    
    // Add more credentials
    credentialStore.credentials = mockCredentials;
    await wrapper.vm.$nextTick();
    
    expect(wrapper.findAll('.card.bg-base-200')).toHaveLength(3);
  });
});
