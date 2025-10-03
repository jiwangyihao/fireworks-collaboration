import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ConfirmDialog from '../ConfirmDialog.vue';

describe('ConfirmDialog.vue', () => {
  it('不显示对话框当show=false', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: false,
        title: 'Test Title',
        message: 'Test Message',
      },
    });

    const dialog = wrapper.find('dialog');
    expect(dialog.attributes('open')).toBeUndefined();
  });

  it('显示对话框当show=true', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test Title',
        message: 'Test Message',
      },
    });

    const dialog = wrapper.find('dialog');
    expect(dialog.attributes('open')).toBeDefined();
  });

  it('显示正确的标题和消息', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Delete Confirmation',
        message: 'Are you sure you want to delete this item?',
      },
    });

    expect(wrapper.text()).toContain('Delete Confirmation');
    expect(wrapper.text()).toContain('Are you sure you want to delete this item?');
  });

  it('显示可选的详情信息', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test message',
        details: 'This action cannot be undone',
      },
    });

    expect(wrapper.text()).toContain('This action cannot be undone');
  });

  it('不显示详情信息当details未提供', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test message',
      },
    });

    const detailsDiv = wrapper.find('.text-sm.text-base-content\\/70');
    expect(detailsDiv.exists()).toBe(false);
  });

  it('显示自定义确认按钮文本', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
        confirmText: 'Yes, Delete',
      },
    });

    expect(wrapper.text()).toContain('Yes, Delete');
  });

  it('显示默认确认按钮文本', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
      },
    });

    expect(wrapper.text()).toContain('确认');
  });

  it('danger变体应用错误按钮样式', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
        variant: 'danger',
      },
    });

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('确认'));
    expect(confirmButton?.classes()).toContain('btn-error');
  });

  it('warning变体应用警告按钮样式', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
        variant: 'warning',
      },
    });

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('确认'));
    expect(confirmButton?.classes()).toContain('btn-warning');
  });

  it('info变体应用信息按钮样式', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
        variant: 'info',
      },
    });

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('确认'));
    expect(confirmButton?.classes()).toContain('btn-info');
  });

  it('点击确认按钮触发confirm事件', async () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
      },
    });

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('确认'));
    await confirmButton?.trigger('click');

    expect(wrapper.emitted('confirm')).toBeTruthy();
    expect(wrapper.emitted('confirm')?.length).toBe(1);
  });

  it('点击取消按钮触发cancel事件', async () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
      },
    });

    const cancelButton = wrapper.findAll('button').find(btn => btn.text().includes('取消'));
    await cancelButton?.trigger('click');

    expect(wrapper.emitted('cancel')).toBeTruthy();
    expect(wrapper.emitted('cancel')?.length).toBe(1);
  });

  it('点击背景区域触发cancel事件', async () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
      },
    });

    const backdrop = wrapper.find('.modal-backdrop button');
    await backdrop.trigger('click');

    expect(wrapper.emitted('cancel')).toBeTruthy();
  });

  it('确认和取消事件不携带参数', async () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Test',
        message: 'Test',
      },
    });

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('确认'));
    const cancelButton = wrapper.findAll('button').find(btn => btn.text().includes('取消'));

    await confirmButton?.trigger('click');
    await cancelButton?.trigger('click');

    expect(wrapper.emitted('confirm')?.[0]).toEqual([]);
    expect(wrapper.emitted('cancel')?.[0]).toEqual([]);
  });

  it('所有props正确传递并显示', () => {
    const wrapper = mount(ConfirmDialog, {
      props: {
        show: true,
        title: 'Complete Title',
        message: 'Complete Message',
        details: 'Complete Details',
        confirmText: 'Custom Confirm',
        variant: 'warning',
      },
    });

    expect(wrapper.text()).toContain('Complete Title');
    expect(wrapper.text()).toContain('Complete Message');
    expect(wrapper.text()).toContain('Complete Details');
    expect(wrapper.text()).toContain('Custom Confirm');

    const confirmButton = wrapper.findAll('button').find(btn => btn.text().includes('Custom Confirm'));
    expect(confirmButton?.classes()).toContain('btn-warning');
  });
});
