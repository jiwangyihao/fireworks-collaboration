import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import DocumentTree from "../DocumentTree.vue";
import type { DocTreeNode } from "../../../api/vitepress";

// Mock Child Component to avoid deep rendering
vi.mock("../DocumentTreeItem.vue", () => ({
  default: {
    props: ["node"],
    template:
      '<li class="mock-item" @click="$emit(\'select\', node)" @contextmenu="$emit(\'contextmenu\', {event: $event, node: node})">{{ node.name }}</li>',
  },
}));

describe("DocumentTree", () => {
  const mockTree: DocTreeNode = {
    name: "root",
    path: "/root",
    nodeType: "folder",
    children: [
      {
        name: "file1.md",
        path: "/root/file1.md",
        nodeType: "file",
        children: null,
        title: null,
        gitStatus: null,
        order: null,
      },
    ],
    title: null,
    gitStatus: null,
    order: null,
  };

  it("renders loading state", () => {
    const wrapper = mount(DocumentTree, {
      props: {
        tree: null,
        loading: true,
      },
    });

    expect(wrapper.find(".loading-spinner").exists()).toBe(true);
  });

  it("renders empty state", () => {
    const wrapper = mount(DocumentTree, {
      props: {
        tree: null,
        loading: false,
      },
    });

    expect(wrapper.text()).toContain("暂无文档");
  });

  it("renders tree items when data is present", () => {
    const wrapper = mount(DocumentTree, {
      props: {
        tree: mockTree,
        loading: false,
      },
    });

    // Check if mock item is rendered for the child
    expect(wrapper.findAll(".mock-item").length).toBe(1);
    expect(wrapper.text()).toContain("file1.md");
  });

  it("propagates select event from child", async () => {
    const wrapper = mount(DocumentTree, {
      props: {
        tree: mockTree,
        loading: false,
      },
    });

    await wrapper.find(".mock-item").trigger("click");
    expect(wrapper.emitted("select")).toBeTruthy();
    const emittedNode = (wrapper.emitted("select")?.[0] as any)[0];
    expect(emittedNode.name).toBe("file1.md");
  });

  it("propagates contextmenu event from child", async () => {
    const wrapper = mount(DocumentTree, {
      props: {
        tree: mockTree,
        loading: false,
      },
    });

    await wrapper.find(".mock-item").trigger("contextmenu");
    expect(wrapper.emitted("contextmenu")).toBeTruthy();
  });
});
