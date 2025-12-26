import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import AvatarGroup from "../AvatarGroup.vue";
import type { AvatarItem } from "../AvatarGroup.vue";

describe("AvatarGroup", () => {
  const mockItems: AvatarItem[] = [
    { name: "User 1", avatarUrl: "https://example.com/avatar1.png" },
    {
      name: "User 2",
      avatarUrl: "https://example.com/avatar2.png",
      url: "https://github.com/user2",
    },
    { name: "User 3", avatarUrl: "https://example.com/avatar3.png" },
  ];

  describe("rendering", () => {
    it("renders all avatars when count is less than max", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems, max: 5 },
      });

      const avatars = wrapper.findAll(".avatar");
      expect(avatars.length).toBe(3);
    });

    it("renders max avatars with overflow indicator", () => {
      const items: AvatarItem[] = Array.from({ length: 8 }, (_, i) => ({
        name: `User ${i + 1}`,
        avatarUrl: `https://example.com/avatar${i + 1}.png`,
      }));

      const wrapper = mount(AvatarGroup, {
        props: { items, max: 5 },
      });

      // 5 avatars + 1 overflow placeholder
      const avatars = wrapper.findAll(".avatar");
      expect(avatars.length).toBe(6);

      // Check overflow count displays correctly
      expect(wrapper.text()).toContain("+3");
    });

    it("does not show overflow indicator when items equal to max", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems, max: 3 },
      });

      expect(wrapper.text()).not.toContain("+");
    });
  });

  describe("links", () => {
    it("renders anchor tag for items with url", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems },
      });

      const links = wrapper.findAll("a");
      expect(links.length).toBe(1);
      expect(links[0].attributes("href")).toBe("https://github.com/user2");
      expect(links[0].attributes("target")).toBe("_blank");
    });

    it("renders div for items without url", () => {
      const itemsWithoutUrl: AvatarItem[] = [
        { name: "User 1", avatarUrl: "https://example.com/avatar1.png" },
      ];

      const wrapper = mount(AvatarGroup, {
        props: { items: itemsWithoutUrl },
      });

      expect(wrapper.find("a").exists()).toBe(false);
    });
  });

  describe("sizes", () => {
    it("applies xs size class", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems, size: "xs" },
      });

      expect(wrapper.find(".w-5").exists()).toBe(true);
    });

    it("applies sm size class by default", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems },
      });

      expect(wrapper.find(".w-6").exists()).toBe(true);
    });

    it("applies md size class", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems, size: "md" },
      });

      expect(wrapper.find(".w-8").exists()).toBe(true);
    });
  });

  describe("accessibility", () => {
    it("sets title attribute for each avatar", () => {
      const wrapper = mount(AvatarGroup, {
        props: { items: mockItems },
      });

      const images = wrapper.findAll("img");
      images.forEach((img, index) => {
        expect(img.attributes("alt")).toBe(mockItems[index].name);
      });
    });
  });
});
