import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import CredentialView from '../CredentialView.vue';
import { useCredentialStore } from '../../../stores/credential';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock child components
vi.mock('../../../components/CredentialForm.vue', () => ({
  default: {
    name: 'CredentialForm',
    template: '<div class="mock-credential-form"></div>',
    emits: ['success', 'cancel'],
  },
}));

vi.mock('../../../components/CredentialList.vue', () => ({
  default: {
    name: 'CredentialList',
    template: '<div class="mock-credential-list"></div>',
    emits: ['edit'],
  },
}));

vi.mock('../../../components/MasterPasswordDialog.vue', () => ({
  default: {
    name: 'MasterPasswordDialog',
    template: '<div class="mock-master-password-dialog"></div>',
    props: ['show', 'isFirstTime'],
    emits: ['close', 'success'],
  },
}));

vi.mock('../../../components/ConfirmDialog.vue', () => ({
  default: {
    name: 'ConfirmDialog',
    template: '<div class="mock-confirm-dialog" v-if="show"><button @click="$emit(\'confirm\')">确认</button><button @click="$emit(\'cancel\')">取消</button></div>',
    props: ['show', 'title', 'message', 'variant'],
    emits: ['confirm', 'cancel'],
  },
}));

describe('CredentialView.vue', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    global.alert = vi.fn();
  });

  describe('初始渲染', () => {
    it('应该正确渲染页面标题', () => {
      const wrapper = mount(CredentialView);
      expect(wrapper.text()).toContain('凭证管理');
    });

    it('应该在需要解锁时显示解锁提示', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      // Set up state to make needsUnlock return true
      store.config = { storage: 'file', auditMode: false };
      store.isUnlocked = false;
      store.credentials = [];
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toContain('凭证存储已加密');
      expect(wrapper.text()).toContain('解锁存储');
    });

    it('应该在解锁后显示凭证管理界面', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      // Set up state to make needsUnlock return false
      store.isUnlocked = true;
      await wrapper.vm.$nextTick();

      expect(wrapper.find('.mock-credential-list').exists()).toBe(true);
    });
  });

  describe('错误提示', () => {
    it('应该显示store中的错误信息', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.error = 'Test error message';
      await wrapper.vm.$nextTick();

      expect(wrapper.find('.alert-error').exists()).toBe(true);
      expect(wrapper.text()).toContain('Test error message');
    });

    it('应该点击关闭按钮清除错误', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      const mockInvoke = invoke as ReturnType<typeof vi.fn>;
      // Mock successful empty response to prevent refresh errors
      mockInvoke.mockResolvedValue([]);

      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.error = 'Test error';
      await wrapper.vm.$nextTick();

      const closeButton = wrapper.find('.alert-error button');
      await closeButton.trigger('click');
      await wrapper.vm.$nextTick();

      expect(store.error).toBeNull();
    });
  });

  describe('过期凭证警告', () => {
    it('应该显示即将过期凭证警告', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      const now = Date.now() / 1000;
      const expiringSoon = now + (5 * 86400); // 5 days from now
      store.credentials = [
        { host: 'github.com', username: 'user1', isExpired: false, expiresAt: expiringSoon } as any,
      ];
      await wrapper.vm.$nextTick();

      expect(wrapper.find('.alert-warning').exists()).toBe(true);
      expect(wrapper.text()).toContain('即将过期提醒');
      expect(wrapper.text()).toContain('1 个凭证即将在 7 天内过期');
    });

    it('应该显示已过期凭证警告', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [
        { host: 'github.com', username: 'user1', isExpired: true } as any,
        { host: 'gitlab.com', username: 'user2', isExpired: true } as any,
      ];
      await wrapper.vm.$nextTick();

      expect(wrapper.find('.alert-error').exists()).toBe(true);
      expect(wrapper.text()).toContain('已过期凭证');
      expect(wrapper.text()).toContain('2 个凭证已过期');
    });

    it('应该在已过期警告中显示清理按钮', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [
        { host: 'github.com', username: 'user1', isExpired: true } as any,
      ];
      await wrapper.vm.$nextTick();

      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      expect(cleanupButton).toBeDefined();
    });
  });

  describe('添加凭证功能', () => {
    it('应该点击添加凭证按钮显示表单', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      await wrapper.vm.$nextTick();

      const addButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('添加凭证')
      );
      await addButton?.trigger('click');

      expect(wrapper.find('.mock-credential-form').exists()).toBe(true);
    });

    it('应该点击取消添加按钮隐藏表单', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      await wrapper.vm.$nextTick();

      // Show form
      const addButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('添加凭证')
      );
      await addButton?.trigger('click');

      // Hide form
      const cancelButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('取消添加')
      );
      await cancelButton?.trigger('click');

      expect(wrapper.find('.mock-credential-form').exists()).toBe(false);
    });
  });

  describe('刷新功能', () => {
    it('应该显示刷新按钮', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      await wrapper.vm.$nextTick();

      const refreshButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('刷新')
      );
      expect(refreshButton).toBeDefined();
    });

    it('应该在加载时禁用刷新按钮', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      store.loading = true;
      await wrapper.vm.$nextTick();

      const refreshButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('刷新')
      );
      expect(refreshButton?.attributes('disabled')).toBeDefined();
    });
  });

  describe('ConfirmDialog 集成 - 清理过期凭证', () => {
    it('应该点击清理过期凭证显示确认对话框', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [
        { host: 'github.com', username: 'user1', isExpired: true } as any,
      ];
      await wrapper.vm.$nextTick();

      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');

      expect(wrapper.find('.mock-confirm-dialog').exists()).toBe(true);
    });

    it('应该在确认对话框中显示正确的消息', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      // Use $patch to ensure reactive updates
      store.$patch({
        isUnlocked: true,
        credentials: [
          { host: 'github.com', username: 'user1', isExpired: true, createdAt: 1000, maskedPassword: '***' } as any,
          { host: 'gitlab.com', username: 'user2', isExpired: true, createdAt: 2000, maskedPassword: '***' } as any,
        ],
      });
      await wrapper.vm.$nextTick();
      await flushPromises(); // Wait for all reactive updates

      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');
      await wrapper.vm.$nextTick();

      const dialog = wrapper.findComponent({ name: 'ConfirmDialog' });
      expect(dialog.props('title')).toBe('清理过期凭证');
      // The message should mention expired credentials and warning text
      const message = dialog.props('message') as string;
      expect(message).toContain('个已过期的凭证');
      expect(message).toContain('此操作不可撤销');
    });

    it('应该使用warning变体的确认对话框', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [{ host: 'github.com', username: 'user1', isExpired: true } as any];
      await wrapper.vm.$nextTick();

      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');

      const dialog = wrapper.findComponent({ name: 'ConfirmDialog' });
      expect(dialog.props('variant')).toBe('warning');
    });

    it('应该点击确认按钮执行清理操作', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [{ host: 'github.com', username: 'user1', isExpired: true } as any];
      store.cleanupExpired = vi.fn().mockResolvedValue(1);
      await wrapper.vm.$nextTick();

      // Show dialog
      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');

      // Confirm cleanup
      const confirmButton = wrapper.find('.mock-confirm-dialog button');
      await confirmButton.trigger('click');
      await flushPromises();

      expect(store.cleanupExpired).toHaveBeenCalled();
      expect(global.alert).toHaveBeenCalledWith('成功清理 1 个过期凭证');
    });

    it('应该点击取消按钮关闭对话框', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [{ host: 'github.com', username: 'user1', isExpired: true } as any];
      await wrapper.vm.$nextTick();

      // Show dialog
      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');

      // Cancel
      const cancelButton = wrapper.findAll('.mock-confirm-dialog button')[1];
      await cancelButton.trigger('click');

      expect(wrapper.find('.mock-confirm-dialog').exists()).toBe(false);
    });

    it('应该在清理失败时不显示成功消息', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.isUnlocked = true;
      store.credentials = [{ host: 'github.com', username: 'user1', isExpired: true } as any];
      store.cleanupExpired = vi.fn().mockRejectedValue(new Error('Cleanup failed'));
      await wrapper.vm.$nextTick();

      // Show dialog and confirm
      const cleanupButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('清理过期凭证')
      );
      await cleanupButton?.trigger('click');

      const confirmButton = wrapper.find('.mock-confirm-dialog button');
      await confirmButton.trigger('click');
      await flushPromises();

      expect(global.alert).not.toHaveBeenCalled();
    });
  });

  describe('导出审计日志功能', () => {
    it('应该显示导出审计日志按钮', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      await wrapper.vm.$nextTick();

      const exportButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('导出审计日志')
      );
      expect(exportButton).toBeDefined();
    });

    it('应该在加载时禁用导出按钮', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();
      store.isUnlocked = true;
      store.loading = true;
      await wrapper.vm.$nextTick();

      const exportButton = wrapper.findAll('button').find(btn =>
        btn.text().includes('导出审计日志')
      );
      expect(exportButton?.attributes('disabled')).toBeDefined();
    });
  });

  describe('主密码对话框', () => {
    it('应该在需要解锁时点击解锁按钮显示主密码对话框', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.config = { storage: 'file', auditMode: false };
      store.isUnlocked = false;
      store.credentials = [];
      await wrapper.vm.$nextTick();

      const unlockButton = wrapper.find('button');
      await unlockButton.trigger('click');

      const dialog = wrapper.findComponent({ name: 'MasterPasswordDialog' });
      expect(dialog.props('show')).toBe(true);
    });

    it('应该在解锁成功后刷新凭证列表', async () => {
      const wrapper = mount(CredentialView);
      const store = useCredentialStore();

      store.config = { storage: 'file', auditMode: false };
      store.isUnlocked = false;
      store.credentials = [];
      store.refresh = vi.fn().mockResolvedValue(undefined);
      await wrapper.vm.$nextTick();

      // Trigger success event from MasterPasswordDialog
      const dialog = wrapper.findComponent({ name: 'MasterPasswordDialog' });
      await dialog.vm.$emit('success');
      await flushPromises();

      expect(store.refresh).toHaveBeenCalled();
    });
  });
});
