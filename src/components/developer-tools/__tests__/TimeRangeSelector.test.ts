import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import TimeRangeSelector from "../observability/TimeRangeSelector.vue";

describe("TimeRangeSelector", () => {
  const mockOptions = [
    { label: "1h", value: "1h" as const },
    { label: "24h", value: "24h" as const },
    { label: "7d", value: "7d" as const },
  ];

  describe("rendering", () => {
    it("renders the component", () => {
      const wrapper = mount(TimeRangeSelector, {
        props: { modelValue: "1h", options: mockOptions },
      });
      expect(wrapper.find(".time-range-selector").exists()).toBe(true);
    });

    it("renders all options as buttons", () => {
      const wrapper = mount(TimeRangeSelector, {
        props: { modelValue: "1h", options: mockOptions },
      });
      const buttons = wrapper.findAll("button");
      expect(buttons.length).toBe(3);
      expect(buttons[0].text()).toBe("1h");
      expect(buttons[1].text()).toBe("24h");
      expect(buttons[2].text()).toBe("7d");
    });
  });

  describe("active state", () => {
    it("applies active styling to selected option", () => {
      const wrapper = mount(TimeRangeSelector, {
        props: { modelValue: "24h", options: mockOptions },
      });
      const buttons = wrapper.findAll("button");

      // Active button should have primary classes
      expect(buttons[1].classes()).toContain("bg-primary");
      expect(buttons[1].classes()).toContain("text-primary-content");

      // Inactive buttons should not have primary classes
      expect(buttons[0].classes()).not.toContain("bg-primary");
      expect(buttons[2].classes()).not.toContain("bg-primary");
    });
  });

  describe("events", () => {
    it("emits update:modelValue when different option clicked", async () => {
      const wrapper = mount(TimeRangeSelector, {
        props: { modelValue: "1h", options: mockOptions },
      });

      await wrapper.findAll("button")[2].trigger("click");

      expect(wrapper.emitted("update:modelValue")).toHaveLength(1);
      expect(wrapper.emitted("update:modelValue")![0]).toEqual(["7d"]);
    });

    it("does not emit when same option clicked", async () => {
      const wrapper = mount(TimeRangeSelector, {
        props: { modelValue: "1h", options: mockOptions },
      });

      await wrapper.findAll("button")[0].trigger("click");

      expect(wrapper.emitted("update:modelValue")).toBeUndefined();
    });
  });
});
