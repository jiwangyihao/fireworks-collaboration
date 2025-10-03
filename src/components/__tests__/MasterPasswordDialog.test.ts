import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import MasterPasswordDialog from '../MasterPasswordDialog.vue';
import { useCredentialStore } from '../../stores/credential';

// Mock Tauri API
vi.mock('../../api/tauri', () => ({
  invoke: vi.fn(),
}));

describe('MasterPasswordDialog.vue', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it('应该在show=false时不渲染', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: false,
      },
    });
    
    expect(wrapper.find('.modal').exists()).toBe(false);
  });

  it('应该在show=true时渲染', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
      },
    });
    
    expect(wrapper.find('.modal').exists()).toBe(true);
  });

  it('应该在首次设置时显示正确的标题', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    expect(wrapper.text()).toContain('设置主密码');
  });

  it('应该在解锁时显示正确的标题', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: false,
      },
    });
    
    expect(wrapper.text()).toContain('解锁凭证存储');
  });

  it('应该在首次设置时显示确认密码字段', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    expect(wrapper.find('input#confirm-password').exists()).toBe(true);
  });

  it('应该在解锁时不显示确认密码字段', () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: false,
      },
    });
    
    expect(wrapper.find('input#confirm-password').exists()).toBe(false);
  });

  it('应该在首次设置时显示密码强度指示器', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('Test123!@#');
    
    expect(wrapper.find('progress').exists()).toBe(true);
    expect(wrapper.text()).toMatch(/弱|中等|强/);
  });

  it('应该在密码不匹配时禁用提交按钮', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('password123');
    await wrapper.find('input#confirm-password').setValue('different');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('设置')
    );
    
    expect(submitBtn?.attributes('disabled')).toBeDefined();
  });

  it('应该在密码匹配时启用提交按钮', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('password123');
    await wrapper.find('input#confirm-password').setValue('password123');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('设置')
    );
    
    expect(submitBtn?.attributes('disabled')).toBeUndefined();
  });

  it('应该在密码少于8字符时禁用提交按钮', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('pass');
    await wrapper.find('input#confirm-password').setValue('pass');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('设置')
    );
    
    expect(submitBtn?.attributes('disabled')).toBeDefined();
  });

  it('应该计算密码强度 - 弱密码', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('pass1234');
    
    // Should show weak or medium strength
    expect(wrapper.text()).toMatch(/弱|中等/);
  });

  it('应该计算密码强度 - 中等密码', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('Password12');
    
    // Should show medium or strong strength
    expect(wrapper.text()).toMatch(/中等|强/);
  });

  it('应该计算密码强度 - 强密码', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    
    await wrapper.find('input#master-password').setValue('P@ssw0rd!123');
    
    expect(wrapper.text()).toContain('强');
  });

  it('应该在首次设置成功后触发success事件', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: true,
      },
    });
    const credentialStore = useCredentialStore();
    
    credentialStore.setPassword = vi.fn().mockResolvedValue(undefined);
    
    await wrapper.find('input#master-password').setValue('password123');
    await wrapper.find('input#confirm-password').setValue('password123');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('设置')
    );
    await submitBtn?.trigger('click');
    
    // Wait for async operations
    await new Promise(resolve => setTimeout(resolve, 50));
    
    expect(wrapper.emitted('success')).toBeTruthy();
  });

  it('应该在解锁成功后触发success事件', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: false,
      },
    });
    const credentialStore = useCredentialStore();
    
    credentialStore.unlock = vi.fn().mockResolvedValue(undefined);
    
    await wrapper.find('input#master-password').setValue('password123');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('解锁')
    );
    await submitBtn?.trigger('click');
    
    // Wait for async operations
    await new Promise(resolve => setTimeout(resolve, 50));
    
    expect(wrapper.emitted('success')).toBeTruthy();
  });

  it('应该在点击取消时触发close事件', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
      },
    });
    
    const cancelBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('取消')
    );
    await cancelBtn?.trigger('click');
    
    expect(wrapper.emitted('close')).toBeTruthy();
  });

  it('应该在操作失败时显示错误消息', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: false,
      },
    });
    const credentialStore = useCredentialStore();
    
    credentialStore.unlock = vi.fn().mockRejectedValue(new Error('密码错误'));
    
    await wrapper.find('input#master-password').setValue('wrongpassword');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('解锁')
    );
    await submitBtn?.trigger('click');
    
    // Wait for async operations
    await new Promise(resolve => setTimeout(resolve, 50));
    
    expect(wrapper.text()).toContain('密码错误');
  });

  it('应该在输入时清除错误消息', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: true,
        isFirstTime: false,
      },
    });
    const credentialStore = useCredentialStore();
    
    credentialStore.unlock = vi.fn().mockRejectedValue(new Error('密码错误'));
    
    await wrapper.find('input#master-password').setValue('wrongpassword');
    
    const submitBtn = wrapper.findAll('button').find((btn) => 
      btn.text().includes('解锁')
    );
    await submitBtn?.trigger('click');
    
    // Wait for async operations
    await new Promise(resolve => setTimeout(resolve, 50));
    
    expect(wrapper.text()).toContain('密码错误');
    
    await wrapper.find('input#master-password').setValue('newpassword');
    
    expect(wrapper.text()).not.toContain('密码错误');
  });

  it('应该在dialog打开时清除旧的输入', async () => {
    const wrapper = mount(MasterPasswordDialog, {
      props: {
        show: false,
      },
    });
    
    // Open dialog
    await wrapper.setProps({ show: true });
    await wrapper.find('input#master-password').setValue('password123');
    
    // Close and reopen
    await wrapper.setProps({ show: false });
    await wrapper.setProps({ show: true });
    
    const passwordInput = wrapper.find('input#master-password');
    expect((passwordInput.element as HTMLInputElement).value).toBe('');
  });
});
