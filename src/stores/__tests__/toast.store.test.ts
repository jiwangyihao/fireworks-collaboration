import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useToastStore } from "../toast";

describe("useToastStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("initial state", () => {
    it("starts with empty toasts array", () => {
      const store = useToastStore();
      expect(store.toasts).toEqual([]);
    });
  });

  describe("add action", () => {
    it("adds a toast with correct properties", () => {
      const store = useToastStore();
      const id = store.add("info", "Test message", 5000);

      expect(store.toasts).toHaveLength(1);
      expect(store.toasts[0].id).toBe(id);
      expect(store.toasts[0].type).toBe("info");
      expect(store.toasts[0].message).toBe("Test message");
      expect(store.toasts[0].duration).toBe(5000);
    });

    it("returns unique incrementing ids", () => {
      const store = useToastStore();
      const id1 = store.add("info", "Message 1");
      const id2 = store.add("info", "Message 2");

      expect(id2).toBeGreaterThan(id1);
    });

    it("auto-removes toast after duration", () => {
      const store = useToastStore();
      store.add("info", "Test message", 3000);

      expect(store.toasts).toHaveLength(1);

      vi.advanceTimersByTime(3000);

      expect(store.toasts).toHaveLength(0);
    });

    it("does not auto-remove toast when duration is 0", () => {
      const store = useToastStore();
      store.add("error", "Persistent error", 0);

      vi.advanceTimersByTime(10000);

      expect(store.toasts).toHaveLength(1);
    });
  });

  describe("remove action", () => {
    it("removes toast by id", () => {
      const store = useToastStore();
      const id1 = store.add("info", "Message 1", 0);
      const id2 = store.add("info", "Message 2", 0);

      store.remove(id1);

      expect(store.toasts).toHaveLength(1);
      expect(store.toasts[0].id).toBe(id2);
    });

    it("does nothing when id not found", () => {
      const store = useToastStore();
      store.add("info", "Message", 0);

      store.remove(99999);

      expect(store.toasts).toHaveLength(1);
    });
  });

  describe("convenience methods", () => {
    it("info() creates info toast with 5000ms duration", () => {
      const store = useToastStore();
      store.info("Info message");

      expect(store.toasts[0].type).toBe("info");
      expect(store.toasts[0].duration).toBe(5000);
    });

    it("info() respects custom duration", () => {
      const store = useToastStore();
      store.info("Info message", 1000);

      expect(store.toasts[0].duration).toBe(1000);
    });

    it("success() creates success toast with 3000ms duration", () => {
      const store = useToastStore();
      store.success("Success message");

      expect(store.toasts[0].type).toBe("success");
      expect(store.toasts[0].duration).toBe(3000);
    });

    it("warning() creates warning toast with 5000ms duration", () => {
      const store = useToastStore();
      store.warning("Warning message");

      expect(store.toasts[0].type).toBe("warning");
      expect(store.toasts[0].duration).toBe(5000);
    });

    it("error() creates error toast with 0 duration (no auto-close)", () => {
      const store = useToastStore();
      store.error("Error message");

      expect(store.toasts[0].type).toBe("error");
      expect(store.toasts[0].duration).toBe(0);
    });
  });

  describe("clear action", () => {
    it("removes all toasts", () => {
      const store = useToastStore();
      store.add("info", "Message 1", 0);
      store.add("success", "Message 2", 0);
      store.add("error", "Message 3", 0);

      expect(store.toasts).toHaveLength(3);

      store.clear();

      expect(store.toasts).toHaveLength(0);
    });
  });

  describe("multiple toasts", () => {
    it("can have multiple toasts at once", () => {
      const store = useToastStore();
      store.info("Info");
      store.success("Success");
      store.warning("Warning");
      store.error("Error");

      expect(store.toasts).toHaveLength(4);
    });

    it("removes toasts in correct order based on duration", () => {
      const store = useToastStore();
      store.add("info", "3s toast", 3000);
      store.add("info", "1s toast", 1000);
      store.add("info", "2s toast", 2000);

      expect(store.toasts).toHaveLength(3);

      vi.advanceTimersByTime(1000);
      expect(store.toasts).toHaveLength(2);

      vi.advanceTimersByTime(1000);
      expect(store.toasts).toHaveLength(1);

      vi.advanceTimersByTime(1000);
      expect(store.toasts).toHaveLength(0);
    });
  });
});
