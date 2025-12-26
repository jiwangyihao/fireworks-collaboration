import { describe, it, expect, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useLogsStore } from "../logs";

describe("useLogsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  describe("initial state", () => {
    it("starts with empty items array", () => {
      const store = useLogsStore();
      expect(store.items).toEqual([]);
    });
  });

  describe("push action", () => {
    it("adds an info log item", () => {
      const store = useLogsStore();
      store.push("info", "Test info message");

      expect(store.items).toHaveLength(1);
      expect(store.items[0].level).toBe("info");
      expect(store.items[0].message).toBe("Test info message");
    });

    it("adds a warn log item", () => {
      const store = useLogsStore();
      store.push("warn", "Test warning message");

      expect(store.items).toHaveLength(1);
      expect(store.items[0].level).toBe("warn");
      expect(store.items[0].message).toBe("Test warning message");
    });

    it("adds an error log item", () => {
      const store = useLogsStore();
      store.push("error", "Test error message");

      expect(store.items).toHaveLength(1);
      expect(store.items[0].level).toBe("error");
      expect(store.items[0].message).toBe("Test error message");
    });

    it("generates unique id for each log item", () => {
      const store = useLogsStore();
      store.push("info", "Message 1");
      store.push("info", "Message 2");

      expect(store.items[0].id).not.toBe(store.items[1].id);
    });

    it("sets time property to current timestamp", () => {
      const store = useLogsStore();
      const before = Date.now();
      store.push("info", "Test message");
      const after = Date.now();

      expect(store.items[0].time).toBeGreaterThanOrEqual(before);
      expect(store.items[0].time).toBeLessThanOrEqual(after);
    });

    it("adds new items to the beginning (unshift)", () => {
      const store = useLogsStore();
      store.push("info", "First message");
      store.push("info", "Second message");

      expect(store.items[0].message).toBe("Second message");
      expect(store.items[1].message).toBe("First message");
    });

    it("limits items to 50", () => {
      const store = useLogsStore();

      // Add 60 items
      for (let i = 0; i < 60; i++) {
        store.push("info", `Message ${i}`);
      }

      expect(store.items).toHaveLength(50);
      // The oldest messages should be removed
      expect(store.items[0].message).toBe("Message 59");
      expect(store.items[49].message).toBe("Message 10");
    });
  });

  describe("clear action", () => {
    it("removes all items", () => {
      const store = useLogsStore();
      store.push("info", "Message 1");
      store.push("warn", "Message 2");
      store.push("error", "Message 3");

      expect(store.items).toHaveLength(3);

      store.clear();

      expect(store.items).toHaveLength(0);
    });
  });
});
