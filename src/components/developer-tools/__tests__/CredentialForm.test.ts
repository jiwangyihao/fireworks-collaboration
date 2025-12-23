import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import CredentialForm from "../CredentialForm.vue";
import { useCredentialStore } from "../../../stores/credential";

// Mock Tauri API
vi.mock("../../../api/tauri", () => ({
  invoke: vi.fn(),
}));

describe("CredentialForm.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("应该渲染表单", () => {
    const wrapper = mount(CredentialForm);

    expect(wrapper.find("input#host").exists()).toBe(true);
    expect(wrapper.find("input#username").exists()).toBe(true);
    expect(wrapper.find("input#password").exists()).toBe(true);
    expect(wrapper.find("input#expires").exists()).toBe(true);
  });

  it("应该在添加模式下显示正确的标题", () => {
    const wrapper = mount(CredentialForm);

    expect(wrapper.text()).toContain("添加凭证");
  });

  it("应该在编辑模式下显示正确的标题", () => {
    const wrapper = mount(CredentialForm, {
      props: {
        editMode: true,
        initialHost: "github.com",
        initialUsername: "user",
      },
    });

    expect(wrapper.text()).toContain("编辑凭证");
  });

  it("应该在编辑模式下禁用host和username字段", () => {
    const wrapper = mount(CredentialForm, {
      props: {
        editMode: true,
        initialHost: "github.com",
        initialUsername: "user",
      },
    });

    const hostInput = wrapper.find("input#host");
    const usernameInput = wrapper.find("input#username");

    expect(hostInput.attributes("disabled")).toBeDefined();
    expect(usernameInput.attributes("disabled")).toBeDefined();
  });

  it("应该在必填字段为空时禁用提交按钮", async () => {
    const wrapper = mount(CredentialForm);

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));

    expect(submitBtn?.attributes("disabled")).toBeDefined();
  });

  it("应该在所有必填字段填写后启用提交按钮", async () => {
    const wrapper = mount(CredentialForm);

    await wrapper.find("input#host").setValue("github.com");
    await wrapper.find("input#username").setValue("testuser");
    await wrapper.find("input#password").setValue("password123");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));

    expect(submitBtn?.attributes("disabled")).toBeUndefined();
  });

  it("应该在提交成功后触发success事件", async () => {
    const wrapper = mount(CredentialForm);
    const credentialStore = useCredentialStore();

    // Mock successful add
    credentialStore.add = vi.fn().mockResolvedValue(undefined);

    await wrapper.find("input#host").setValue("github.com");
    await wrapper.find("input#username").setValue("testuser");
    await wrapper.find("input#password").setValue("password123");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));
    await submitBtn?.trigger("click");

    // Wait for async operations
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(wrapper.emitted("success")).toBeTruthy();
  });

  it("应该在点击取消时触发cancel事件", async () => {
    const wrapper = mount(CredentialForm);

    const cancelBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("取消"));
    await cancelBtn?.trigger("click");

    expect(wrapper.emitted("cancel")).toBeTruthy();
  });

  it("应该在提交失败时显示错误消息", async () => {
    const wrapper = mount(CredentialForm);
    const credentialStore = useCredentialStore();

    // Mock failed add
    credentialStore.add = vi.fn().mockRejectedValue(new Error("添加失败"));

    await wrapper.find("input#host").setValue("github.com");
    await wrapper.find("input#username").setValue("testuser");
    await wrapper.find("input#password").setValue("password123");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));
    await submitBtn?.trigger("click");

    // Wait for async operations
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(wrapper.text()).toContain("添加失败");
  });

  it("应该在输入时清除错误消息", async () => {
    const wrapper = mount(CredentialForm);
    const credentialStore = useCredentialStore();

    // Mock failed add
    credentialStore.add = vi.fn().mockRejectedValue(new Error("添加失败"));

    await wrapper.find("input#host").setValue("github.com");
    await wrapper.find("input#username").setValue("testuser");
    await wrapper.find("input#password").setValue("password123");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));
    await submitBtn?.trigger("click");

    // Wait for async operations
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(wrapper.text()).toContain("添加失败");

    // Input should clear error
    await wrapper.find("input#host").setValue("updated.com");

    expect(wrapper.text()).not.toContain("添加失败");
  });

  it("应该在编辑模式下调用update而不是add", async () => {
    const wrapper = mount(CredentialForm, {
      props: {
        editMode: true,
        initialHost: "github.com",
        initialUsername: "user",
      },
    });
    const credentialStore = useCredentialStore();

    // Mock successful update
    credentialStore.update = vi.fn().mockResolvedValue(undefined);

    await wrapper.find("input#password").setValue("newpassword");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));
    await submitBtn?.trigger("click");

    // Wait for async operations
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(credentialStore.update).toHaveBeenCalled();
  });

  it("应该正确处理过期天数输入", async () => {
    const wrapper = mount(CredentialForm);
    const credentialStore = useCredentialStore();

    credentialStore.add = vi.fn().mockResolvedValue(undefined);

    await wrapper.find("input#host").setValue("github.com");
    await wrapper.find("input#username").setValue("testuser");
    await wrapper.find("input#password").setValue("password123");
    await wrapper.find("input#expires").setValue("90");

    const submitBtn = wrapper
      .findAll("button")
      .find((btn) => btn.text().includes("保存"));
    await submitBtn?.trigger("click");

    // Wait for async operations
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(credentialStore.add).toHaveBeenCalledWith(
      expect.objectContaining({
        expiresInDays: 90,
      })
    );
  });
});
