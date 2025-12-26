import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import ConfirmModal from "../ConfirmModal.vue";

describe("ConfirmModal", () => {
  // Mock dialog methods
  beforeEach(() => {
    HTMLDialogElement.prototype.showModal = vi.fn();
    HTMLDialogElement.prototype.close = vi.fn();
  });

  describe("rendering", () => {
    it("renders dialog element", () => {
      const wrapper = mount(ConfirmModal);
      expect(wrapper.find("dialog").exists()).toBe(true);
    });

    it("renders default title", () => {
      const wrapper = mount(ConfirmModal);
      expect(wrapper.find("h3").text()).toBe("确认");
    });

    it("renders custom title", () => {
      const wrapper = mount(ConfirmModal, {
        props: { title: "Delete Item" },
      });

      expect(wrapper.find("h3").text()).toBe("Delete Item");
    });

    it("renders slot content", () => {
      const wrapper = mount(ConfirmModal, {
        slots: { default: "<p>Are you sure?</p>" },
      });

      expect(wrapper.text()).toContain("Are you sure?");
    });
  });

  describe("buttons", () => {
    it("renders default button texts", () => {
      const wrapper = mount(ConfirmModal);
      const buttons = wrapper.findAll(".modal-action button");

      expect(buttons[0].text()).toBe("取消");
      expect(buttons[1].text()).toBe("确认");
    });

    it("renders custom button texts", () => {
      const wrapper = mount(ConfirmModal, {
        props: { confirmText: "Delete", cancelText: "Keep" },
      });

      const buttons = wrapper.findAll(".modal-action button");
      expect(buttons[0].text()).toBe("Keep");
      expect(buttons[1].text()).toBe("Delete");
    });

    it("applies correct button variant class", () => {
      const wrapper = mount(ConfirmModal, {
        props: { confirmVariant: "error" },
      });

      const confirmBtn = wrapper.findAll(".modal-action button")[1];
      expect(confirmBtn.classes()).toContain("btn-error");
    });

    it("applies primary variant by default", () => {
      const wrapper = mount(ConfirmModal);

      const confirmBtn = wrapper.findAll(".modal-action button")[1];
      expect(confirmBtn.classes()).toContain("btn-primary");
    });
  });

  describe("events", () => {
    it("emits confirm and update:modelValue on confirm", async () => {
      const wrapper = mount(ConfirmModal, {
        props: { modelValue: true },
      });

      const confirmBtn = wrapper.findAll(".modal-action button")[1];
      await confirmBtn.trigger("click");

      expect(wrapper.emitted("confirm")).toHaveLength(1);
      expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
    });

    it("emits cancel and update:modelValue on cancel", async () => {
      const wrapper = mount(ConfirmModal, {
        props: { modelValue: true },
      });

      const cancelBtn = wrapper.findAll(".modal-action button")[0];
      await cancelBtn.trigger("click");

      expect(wrapper.emitted("cancel")).toHaveLength(1);
      expect(wrapper.emitted("update:modelValue")).toEqual([[false]]);
    });
  });

  describe("dialog control", () => {
    it("calls showModal when modelValue becomes true", async () => {
      const wrapper = mount(ConfirmModal, {
        props: { modelValue: false },
      });

      await wrapper.setProps({ modelValue: true });
      expect(HTMLDialogElement.prototype.showModal).toHaveBeenCalled();
    });

    it("calls close when modelValue becomes false", async () => {
      const wrapper = mount(ConfirmModal, {
        props: { modelValue: true },
      });

      await wrapper.setProps({ modelValue: false });
      expect(HTMLDialogElement.prototype.close).toHaveBeenCalled();
    });
  });
});
