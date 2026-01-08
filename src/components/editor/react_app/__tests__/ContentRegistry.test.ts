import { describe, it, expect, vi, beforeEach } from "vitest";
import { ContentRegistry } from "../ContentRegistry";

describe("ContentRegistry", () => {
  let registry: ContentRegistry;

  beforeEach(() => {
    registry = new ContentRegistry();
  });

  describe("Registration", () => {
    it("should register and retrieve content type", () => {
      registry.register("testBlock", {
        label: "Test Block",
        icon: null,
      });

      const config = registry.get("testBlock");
      expect(config.label).toBe("Test Block");
      expect(config.supportedStyles).toContain("bold"); // Default inherited
    });

    it("should handle heading level aliases", () => {
      const h1 = registry.get("heading-1");
      expect(h1.label).toBe("标题 1");
      expect(h1.icon).toBeDefined();
    });

    it("should return default config for unknown types", () => {
      const unknown = registry.get("unknown");
      expect(unknown.label).toBe("unknown");
    });
  });

  describe("Executors", () => {
    it("should register and execute actions", () => {
      const executor = {
        execute: vi.fn(),
        isActive: vi.fn().mockReturnValue(true),
        getValue: vi.fn().mockReturnValue("value"),
      };

      registry.registerExecutor("block-1", "action-1", executor);

      // Verify retrieval
      expect(registry.getExecutor("block-1", "action-1")).toBe(executor);

      // Verify execution
      registry.executeAction("block-1", "action-1", "payload");
      expect(executor.execute).toHaveBeenCalledWith("payload");

      // Verify state checks
      expect(registry.isActionActive("block-1", "action-1")).toBe(true);
      expect(registry.getActionValue("block-1", "action-1")).toBe("value");
    });

    it("should unregister executors", () => {
      const executor = { execute: vi.fn(), isActive: vi.fn() };
      registry.registerExecutor("block-1", "action-1", executor);

      registry.unregisterExecutor("block-1", "action-1");
      expect(registry.getExecutor("block-1", "action-1")).toBeUndefined();

      registry.registerExecutor("block-1", "action-2", executor);
      registry.unregisterExecutors("block-1");
      expect(registry.getExecutor("block-1", "action-2")).toBeUndefined();
    });
  });

  describe("SlashMenu", () => {
    it("should aggregate slash menu items", () => {
      registry.register("block-A", {
        slashMenuItems: [{ id: "A1", title: "Item A1", group: "G1" } as any],
      });
      registry.register("block-B", {
        slashMenuItems: [{ id: "B1", title: "Item B1", group: "G1" } as any],
      });

      const items = registry.getSlashMenuItems();
      expect(items).toHaveLength(2); // A1, B1 (plus defaults if any? no, new instance has defaults)
      // Wait, new ContentRegistry() calls initDefaults().
      // initDefaults registers paragraph, heading, etc. BUT they assume empty slashMenuItems by default in ContentRegistry code?
      // Let's check ContentRegistry defaults: DEFAULT_CONTENT_TYPE slashMenuItems: []
      // And initDefaults does not set slashMenu items explicitly in the shown code snippet.
      // So mainly strictly my registered ones.

      expect(items.find((i) => i.id === "A1")).toBeDefined();
      expect(items.find((i) => i.id === "B1")).toBeDefined();
    });
  });

  describe("Active Inline", () => {
    it("should manage active inline state", () => {
      const executor = { execute: vi.fn(), isActive: vi.fn() };
      registry.setActiveInline("inline-type", { action: executor });

      expect(registry.getActiveInline()?.type).toBe("inline-type");

      // Execute inline action
      registry.executeInlineAction("action", "val");
      expect(executor.execute).toHaveBeenCalledWith("val");

      // Clear
      registry.setActiveInline(null);
      expect(registry.getActiveInline()).toBeNull();
    });
  });
});
