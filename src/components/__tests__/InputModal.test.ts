import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import InputModal from "../InputModal.vue";
import BaseModal from "../BaseModal.vue";

// Mock BaseModal to avoid dialog issues in test environment
vi.mock("../BaseModal.vue", () => ({
  default: {
    name: "BaseModal",
    props: ["modelValue", "title", "confirmText", "cancelText"],
    template: `
      <div v-if="modelValue" class="mock-base-modal">
        <h3>{{ title }}</h3>
        <slot></slot>
        <button class="confirm-btn" @click="$emit('confirm')">{{ confirmText }}</button>
        <button class="cancel-btn" @click="$emit('cancel')">{{ cancelText }}</button>
      </div>
    `,
  },
}));

describe("InputModal", () => {
  it("renders with correct props", () => {
    const wrapper = mount(InputModal, {
      props: {
        modelValue: true,
        title: "Input Name",
        placeholder: "Enter name",
        confirmText: "Save",
      },
    });

    expect(wrapper.find("h3").text()).toBe("Input Name");
    const input = wrapper.find("input");
    expect(input.attributes("placeholder")).toBe("Enter name");
    expect(wrapper.find(".confirm-btn").text()).toBe("Save");
  });

  it("initializes with default value", async () => {
    const wrapper = mount(InputModal, {
      props: {
        modelValue: true,
        defaultValue: "Initial Value",
      },
    });

    // Watcher needs to trigger
    await wrapper.setProps({ modelValue: false });
    await wrapper.setProps({ modelValue: true });

    const input = wrapper.find("input");
    expect((input.element as HTMLInputElement).value).toBe("Initial Value");
  });

  it("emits confirm with value on button click", async () => {
    const wrapper = mount(InputModal, {
      props: {
        modelValue: true,
      },
    });

    await wrapper.find("input").setValue("New Value");
    await wrapper.find(".confirm-btn").trigger("click");

    expect(wrapper.emitted("confirm")).toHaveLength(1);
    expect(wrapper.emitted("confirm")?.[0]).toEqual(["New Value"]);
    expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
  });

  it("emits confirm with value on enter key", async () => {
    const wrapper = mount(InputModal, {
      props: {
        modelValue: true,
      },
    });

    const input = wrapper.find("input");
    await input.setValue("Enter Value");
    await input.trigger("keyup.enter");

    expect(wrapper.emitted("confirm")).toHaveLength(1);
    expect(wrapper.emitted("confirm")?.[0]).toEqual(["Enter Value"]);
    expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
  });

  it("does not emit confirm if input is empty", async () => {
    const wrapper = mount(InputModal, {
      props: {
        modelValue: true,
      },
    });

    await wrapper.find("input").setValue("   ");
    await wrapper.find(".confirm-btn").trigger("click");

    expect(wrapper.emitted("confirm")).toBeFalsy();
  });
});
