import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import DocumentTreeItem from "../DocumentTreeItem.vue";
import type { DocTreeNode } from "../../../api/vitepress";

// Mock BaseIcon
vi.mock("../BaseIcon.vue", () => ({
  default: {
    template: '<span class="base-icon"></span>',
  },
}));

describe("DocumentTreeItem", () => {
  const fileNode: DocTreeNode = {
    name: "test.md",
    path: "/path/test.md",
    nodeType: "file",
    title: "Test File",
    children: null,
    gitStatus: "clean",
    order: 1,
  };

  const folderNode: DocTreeNode = {
    name: "docs",
    path: "/path/docs",
    nodeType: "folder",
    title: "Documentation",
    children: [fileNode],
    gitStatus: "clean",
    order: 0,
  };

  it("renders file node correctly", () => {
    const wrapper = mount(DocumentTreeItem, {
      props: {
        node: fileNode,
      },
    });

    expect(wrapper.find("a").exists()).toBe(true);
    expect(wrapper.text()).toContain("Test File"); // Uses title preference
  });

  it("renders folder node and expands", async () => {
    const wrapper = mount(DocumentTreeItem, {
      props: {
        node: folderNode,
      },
    });

    expect(wrapper.find("details").exists()).toBe(true);
    // Summary click triggers toggle
    await wrapper.find("summary").trigger("click");
    expect((wrapper.vm as any).isExpanded).toBe(true);
  });

  it("emits select event for file", async () => {
    const wrapper = mount(DocumentTreeItem, {
      props: {
        node: fileNode,
      },
    });

    await wrapper.find("a").trigger("click");
    expect(wrapper.emitted("select")).toBeTruthy();
    expect(wrapper.emitted("select")?.[0]).toEqual([fileNode]);
  });

  it("emits contextmenu event", async () => {
    const wrapper = mount(DocumentTreeItem, {
      props: {
        node: fileNode,
      },
    });

    await wrapper.find("a").trigger("contextmenu");
    expect(wrapper.emitted("contextmenu")).toBeTruthy();
    const payload = (wrapper.emitted("contextmenu")?.[0] as any)[0];
    expect(payload.node).toEqual(fileNode);
  });

  it("shows git status", () => {
    const modifiedNode = { ...fileNode, gitStatus: "modified" as const };
    const wrapper = mount(DocumentTreeItem, {
      props: {
        node: modifiedNode,
      },
    });

    // Check availability of dot or badge
    // File uses dot class
    expect(wrapper.find(".bg-warning").exists()).toBe(true);
  });
});
