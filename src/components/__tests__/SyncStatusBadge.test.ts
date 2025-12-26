import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import SyncStatusBadge from "../SyncStatusBadge.vue";
import BaseBadge from "../BaseBadge.vue";
import BaseIcon from "../BaseIcon.vue";

describe("SyncStatusBadge", () => {
  describe("synced state", () => {
    it("shows synced badge when fully synced", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: {
          ahead: 0,
          behind: 0,
          trackingBranch: "origin/main",
        },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      expect(badges.length).toBe(1);
      expect(badges[0].props("variant")).toBe("success");
      expect(wrapper.text()).toContain("已同步");
    });

    it("does not show synced badge without tracking branch", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: {
          ahead: 0,
          behind: 0,
          trackingBranch: null,
        },
      });

      expect(wrapper.text()).not.toContain("已同步");
    });

    it("does not show synced badge when ahead", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: {
          ahead: 1,
          behind: 0,
          trackingBranch: "origin/main",
        },
      });

      expect(wrapper.text()).not.toContain("已同步");
    });
  });

  describe("ahead state", () => {
    it("shows ahead badge when ahead > 0", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 3, behind: 0 },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      const aheadBadge = badges.find((b) => b.props("variant") === "info");
      expect(aheadBadge).toBeDefined();
      expect(wrapper.text()).toContain("3 ahead");
    });

    it("uses info variant for ahead badge", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 1 },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      expect(badges.some((b) => b.props("variant") === "info")).toBe(true);
    });

    it("shows arrow-up icon for ahead", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 1 },
      });

      const icons = wrapper.findAllComponents(BaseIcon);
      expect(icons.some((i) => i.props("icon") === "lucide--arrow-up")).toBe(
        true
      );
    });
  });

  describe("behind state", () => {
    it("shows behind badge when behind > 0", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 0, behind: 5 },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      const behindBadge = badges.find((b) => b.props("variant") === "warning");
      expect(behindBadge).toBeDefined();
      expect(wrapper.text()).toContain("5 behind");
    });

    it("uses warning variant for behind badge", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { behind: 1 },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      expect(badges.some((b) => b.props("variant") === "warning")).toBe(true);
    });

    it("shows arrow-down icon for behind", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { behind: 1 },
      });

      const icons = wrapper.findAllComponents(BaseIcon);
      expect(icons.some((i) => i.props("icon") === "lucide--arrow-down")).toBe(
        true
      );
    });
  });

  describe("combined states", () => {
    it("shows both ahead and behind badges", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 2, behind: 3 },
      });

      expect(wrapper.text()).toContain("2 ahead");
      expect(wrapper.text()).toContain("3 behind");

      const badges = wrapper.findAllComponents(BaseBadge);
      expect(badges.length).toBe(2);
    });

    it("shows synced, no ahead, no behind when all zero with tracking", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 0, behind: 0, trackingBranch: "origin/main" },
      });

      const badges = wrapper.findAllComponents(BaseBadge);
      expect(badges.length).toBe(1);
      expect(wrapper.text()).toContain("已同步");
    });
  });

  describe("default values", () => {
    it("defaults ahead to 0", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { behind: 1 },
      });

      expect(wrapper.text()).not.toContain("ahead");
    });

    it("defaults behind to 0", () => {
      const wrapper = mount(SyncStatusBadge, {
        props: { ahead: 1 },
      });

      expect(wrapper.text()).not.toContain("behind");
    });
  });
});
