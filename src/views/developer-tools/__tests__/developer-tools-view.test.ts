import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createRouter, createMemoryHistory } from "vue-router";
import DeveloperToolsView from "../DeveloperToolsView.vue";

// Create a mock router
const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    { path: "/", component: { template: "<div />" } },
    { path: "/credentials", component: { template: "<div />" } },
    { path: "/workspace", component: { template: "<div />" } },
    { path: "/test", component: { template: "<div />" } },
    { path: "/git", component: { template: "<div />" } },
    { path: "/http-tester", component: { template: "<div />" } },
    { path: "/ip-pool", component: { template: "<div />" } },
    { path: "/observability", component: { template: "<div />" } },
  ],
});

describe("DeveloperToolsView", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe("rendering", () => {
    it("renders the page header", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.find("h1").text()).toBe("开发人员调试工具");
    });

    it("renders description text", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("在这里可以快速进入常用的开发调试页面");
    });

    it("renders tool cards grid", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      const cards = wrapper.findAll(".card");
      expect(cards.length).toBeGreaterThan(0);
    });
  });

  describe("tool entries", () => {
    it("includes credentials management tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("凭据管理");
    });

    it("includes workspace tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("工作区");
    });

    it("includes GitHub Actions debug tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("GitHub Actions 调试");
    });

    it("includes Git panel tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("Git 面板");
    });

    it("includes HTTP tester tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("HTTP 测试");
    });

    it("includes IP pool lab tool", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      expect(wrapper.text()).toContain("IP 池实验室");
    });
  });

  describe("navigation links", () => {
    it("has correct router-link paths", async () => {
      const wrapper = mount(DeveloperToolsView, {
        global: {
          plugins: [router],
        },
      });

      const links = wrapper.findAllComponents({ name: "RouterLink" });
      const paths = links.map((link) => link.props("to"));

      expect(paths).toContain("/credentials");
      expect(paths).toContain("/workspace");
      expect(paths).toContain("/test");
      expect(paths).toContain("/git");
      expect(paths).toContain("/http-tester");
      expect(paths).toContain("/ip-pool");
    });
  });
});
