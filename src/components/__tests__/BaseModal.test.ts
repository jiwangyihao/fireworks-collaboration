import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseModal from "../BaseModal.vue";

describe("BaseModal", () => {
  it("renders with default props", () => {
    const wrapper = mount(BaseModal, {
      props: {
        modelValue: true,
      },
    });

    expect(wrapper.find("dialog").exists()).toBe(true);
    expect(wrapper.find("h3").text()).toBe("提示");
    expect(wrapper.find(".btn-primary").exists()).toBe(true);
  });

  it("renders custom title and button text", () => {
    const wrapper = mount(BaseModal, {
      props: {
        modelValue: true,
        title: "Custom Title",
        confirmText: "Yes",
        cancelText: "No",
      },
    });

    expect(wrapper.find("h3").text()).toBe("Custom Title");
    expect(wrapper.findAll("button").length).toBeGreaterThan(0);
    // Note: finding by text content in buttons is specific to impl
    const buttons = wrapper.findAll("button");
    const texts = buttons.map((b) => b.text());
    expect(texts).toContain("Yes");
    expect(texts).toContain("No");
  });

  it("emits events correctly", async () => {
    const wrapper = mount(BaseModal, {
      props: {
        modelValue: true,
      },
    });

    // Verify confirm
    await wrapper.find(".btn-primary").trigger("click");
    expect(wrapper.emitted("confirm")).toBeTruthy();

    // Verify cancel
    await wrapper.find(".btn-ghost").trigger("click");
    expect(wrapper.emitted("cancel")).toBeTruthy();
    expect(wrapper.emitted("update:modelValue")).toBeTruthy();
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([false]);
  });
});
