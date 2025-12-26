import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import GlobalToast from "../GlobalToast.vue";
import { useToastStore } from "../../stores/toast";

describe("GlobalToast", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe("rendering", () => {
    it("renders toast container", () => {
      const wrapper = mount(GlobalToast);
      expect(wrapper.find(".toast").exists()).toBe(true);
      expect(wrapper.classes()).toContain("toast-end");
      expect(wrapper.classes()).toContain("toast-bottom");
    });

    it("renders no toasts initially", () => {
      const wrapper = mount(GlobalToast);
      expect(wrapper.findAll(".alert").length).toBe(0);
    });
  });

  describe("toast display", () => {
    it("renders toasts from store", async () => {
      const toastStore = useToastStore();
      toastStore.add("info", "Test message", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.findAll(".alert").length).toBe(1);
      expect(wrapper.text()).toContain("Test message");
    });

    it("renders multiple toasts", async () => {
      const toastStore = useToastStore();
      toastStore.add("info", "Message 1", 0);
      toastStore.add("success", "Message 2", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.findAll(".alert").length).toBe(2);
    });
  });

  describe("alert types", () => {
    it("applies success alert class", () => {
      const toastStore = useToastStore();
      toastStore.add("success", "Success!", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.find(".alert").classes()).toContain("alert-success");
    });

    it("applies warning alert class", () => {
      const toastStore = useToastStore();
      toastStore.add("warning", "Warning!", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.find(".alert").classes()).toContain("alert-warning");
    });

    it("applies error alert class", () => {
      const toastStore = useToastStore();
      toastStore.add("error", "Error!", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.find(".alert").classes()).toContain("alert-error");
    });

    it("applies info alert class by default", () => {
      const toastStore = useToastStore();
      toastStore.add("info", "Info!", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.find(".alert").classes()).toContain("alert-info");
    });
  });

  describe("dismiss functionality", () => {
    it("removes toast when close button is clicked", async () => {
      const toastStore = useToastStore();
      toastStore.add("info", "Test message", 0);

      const wrapper = mount(GlobalToast);
      expect(wrapper.findAll(".alert").length).toBe(1);

      await wrapper.find("button").trigger("click");
      expect(wrapper.findAll(".alert").length).toBe(0);
    });
  });
});
