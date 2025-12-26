import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import LanguageBar from "../LanguageBar.vue";

describe("LanguageBar", () => {
  const mockLanguages = {
    TypeScript: 5000,
    JavaScript: 3000,
    Vue: 2000,
  };

  describe("rendering", () => {
    it("renders progress bar container", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages },
      });

      expect(wrapper.find(".h-2.rounded-full").exists()).toBe(true);
    });

    it("does not render when languages is empty", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: {} },
      });

      expect(wrapper.find(".h-2").exists()).toBe(false);
    });

    it("renders segments for each language", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages },
      });

      const segments = wrapper.findAll(".h-full");
      expect(segments.length).toBe(3);
    });
  });

  describe("percentages", () => {
    it("calculates correct percentage widths", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages },
      });

      const segments = wrapper.findAll(".h-full");

      // TypeScript: 5000/10000 = 50%
      expect(segments[0].attributes("style")).toContain("width: 50%");
      // JavaScript: 3000/10000 = 30%
      expect(segments[1].attributes("style")).toContain("width: 30%");
      // Vue: 2000/10000 = 20%
      expect(segments[2].attributes("style")).toContain("width: 20%");
    });

    it("sets title attribute with language and percentage", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages },
      });

      const segments = wrapper.findAll(".h-full");
      expect(segments[0].attributes("title")).toBe("TypeScript: 50%");
    });
  });

  describe("colors", () => {
    it("applies correct color for TypeScript", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: { TypeScript: 100 } },
      });

      expect(wrapper.find(".bg-blue-500").exists()).toBe(true);
    });

    it("applies correct color for JavaScript", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: { JavaScript: 100 } },
      });

      expect(wrapper.find(".bg-yellow-400").exists()).toBe(true);
    });

    it("applies correct color for Vue", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: { Vue: 100 } },
      });

      expect(wrapper.find(".bg-purple-500").exists()).toBe(true);
    });

    it("applies default color for unknown languages", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: { UnknownLang: 100 } },
      });

      expect(wrapper.find(".bg-primary").exists()).toBe(true);
    });
  });

  describe("legend", () => {
    it("does not show legend by default", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages },
      });

      expect(wrapper.findAll(".flex.flex-wrap").length).toBe(0);
    });

    it("shows legend when showLegend is true", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: mockLanguages, showLegend: true },
      });

      expect(wrapper.text()).toContain("TypeScript 50%");
      expect(wrapper.text()).toContain("JavaScript 30%");
      expect(wrapper.text()).toContain("Vue 20%");
    });

    it("legend items have color dots", () => {
      const wrapper = mount(LanguageBar, {
        props: { languages: { TypeScript: 100 }, showLegend: true },
      });

      expect(wrapper.find(".w-1\\.5.h-1\\.5.rounded-full").exists()).toBe(true);
    });
  });
});
