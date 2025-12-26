import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { formatNumber, relativeTime } from "../format";

describe("format utils", () => {
  describe("formatNumber", () => {
    it("returns number as string when less than 1000", () => {
      expect(formatNumber(0)).toBe("0");
      expect(formatNumber(1)).toBe("1");
      expect(formatNumber(999)).toBe("999");
    });

    it("formats number with k suffix when 1000 or more", () => {
      expect(formatNumber(1000)).toBe("1.0k");
      expect(formatNumber(1500)).toBe("1.5k");
      expect(formatNumber(2000)).toBe("2.0k");
    });

    it("formats large numbers correctly", () => {
      expect(formatNumber(10000)).toBe("10.0k");
      expect(formatNumber(12345)).toBe("12.3k");
      expect(formatNumber(999999)).toBe("1000.0k");
    });

    it("handles decimal rounding", () => {
      expect(formatNumber(1234)).toBe("1.2k");
      expect(formatNumber(1256)).toBe("1.3k");
      expect(formatNumber(1999)).toBe("2.0k");
    });
  });

  describe("relativeTime", () => {
    beforeEach(() => {
      // Mock Date.now() to a fixed time for consistent testing
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2024-12-26T12:00:00"));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("returns '今天' for today's date", () => {
      expect(relativeTime("2024-12-26")).toBe("今天");
    });

    it("returns '昨天' for yesterday", () => {
      expect(relativeTime("2024-12-25")).toBe("昨天");
    });

    it("returns 'X 天前' for days within a week", () => {
      expect(relativeTime("2024-12-24")).toBe("2 天前");
      expect(relativeTime("2024-12-23")).toBe("3 天前");
      expect(relativeTime("2024-12-21")).toBe("5 天前");
      expect(relativeTime("2024-12-20")).toBe("6 天前");
    });

    it("returns 'X 周前' for days within a month", () => {
      expect(relativeTime("2024-12-19")).toBe("1 周前");
      expect(relativeTime("2024-12-12")).toBe("2 周前");
      expect(relativeTime("2024-12-05")).toBe("3 周前");
    });

    it("returns 'X 个月前' for days within a year", () => {
      expect(relativeTime("2024-11-26")).toBe("1 个月前");
      expect(relativeTime("2024-10-26")).toBe("2 个月前");
      expect(relativeTime("2024-06-26")).toBe("6 个月前");
    });

    it("returns 'X 年前' for dates more than a year ago", () => {
      expect(relativeTime("2023-12-26")).toBe("1 年前");
      expect(relativeTime("2022-12-26")).toBe("2 年前");
      expect(relativeTime("2020-12-26")).toBe("4 年前");
    });
  });
});
