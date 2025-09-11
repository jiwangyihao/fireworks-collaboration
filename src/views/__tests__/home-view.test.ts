import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import HomeView from "../HomeView.vue";

describe("views/HomeView", () => {
  beforeEach(() => {
    (invoke as any).mockReset();
  });

  it("提交表单后调用 greet 并显示返回文本", async () => {
    (invoke as any).mockResolvedValueOnce("Hello, Tester!");

    const wrapper = mount(HomeView);

    const input = wrapper.get("#greet-input");
    await input.setValue("Tester");

    await wrapper.get("form").trigger("submit.prevent");
    await flushPromises();

    expect(invoke as any).toHaveBeenCalledWith("greet", { name: "Tester" });
    expect(wrapper.text()).toContain("Hello, Tester!");
  });
});
