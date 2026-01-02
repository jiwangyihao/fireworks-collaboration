import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import DevServerStatus from "../DevServerStatus.vue";
import * as api from "../../../api/vitepress";
import { useToastStore } from "../../../stores/toast";
import { createTestingPinia } from "@pinia/testing";
import { nextTick } from "vue";

// Mock API
vi.mock("../../../api/vitepress", () => ({
  startDevServer: vi.fn(),
  stopDevServer: vi.fn(),
}));

// Mock @tauri-apps/api/event
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock BaseIcon
vi.mock("../BaseIcon.vue", () => ({
  default: {
    template: '<span class="base-icon"></span>',
  },
}));

describe("DevServerStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const mountComponent = () => {
    return mount(DevServerStatus, {
      props: {
        projectPath: "/test/path",
      },
      global: {
        plugins: [
          createTestingPinia({
            createSpy: vi.fn,
          }),
        ],
      },
    });
  };

  it("initializes with stopped state", () => {
    const wrapper = mountComponent();
    expect(wrapper.text()).toContain("Stopped");
    expect(wrapper.find(".text-base-content\\/50").exists()).toBe(true);
  });

  it("handles start server success", async () => {
    const wrapper = mountComponent();
    const mockInfo = {
      url: "http://localhost:5173",
      port: 5173,
      processId: 1234,
      status: "running",
    };

    vi.mocked(api.startDevServer).mockResolvedValue(mockInfo as any);

    await wrapper.find("button").trigger("click"); // Start button

    // Wait for async
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();

    expect(api.startDevServer).toHaveBeenCalledWith("/test/path");
    expect(wrapper.text()).toContain("Running");
    expect(wrapper.find(".text-success").exists()).toBe(true);
  });

  it("handles start server failure", async () => {
    const wrapper = mountComponent();
    const toastStore = useToastStore();
    vi.mocked(api.startDevServer).mockRejectedValue(new Error("Start failed"));

    await wrapper.find("button").trigger("click");

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();

    expect(wrapper.text()).toContain("Error");
    expect(wrapper.find(".text-error").exists()).toBe(true);
    expect(toastStore.error).toHaveBeenCalledWith(
      expect.stringContaining("Failed to start Dev Server")
    );
  });

  it("handles stop server", async () => {
    const wrapper = mountComponent();
    // Verify stop button shows when running
    // Manually set state for test since it's local ref
    (wrapper.vm as any).status = "running";
    (wrapper.vm as any).processId = 1234;
    await nextTick();

    expect(wrapper.text()).toContain("Stop");

    vi.mocked(api.stopDevServer).mockResolvedValue(undefined);

    await wrapper.find("button").trigger("click"); // Stop button is first now? No, need to find by text or class logic
    // Logic: v-if="status === 'running'..." shows stop button.
    // The previous test assumed start button was visible.
    // Let's use simpler query if possible.
    const stopBtn = wrapper
      .findAll("button")
      .find((b) => b.text().includes("Stop"));
    await stopBtn?.trigger("click");

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();

    expect(api.stopDevServer).toHaveBeenCalledWith(1234);
    expect(wrapper.text()).toContain("Stopped");
  });
});
