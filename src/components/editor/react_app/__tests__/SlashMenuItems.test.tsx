import { describe, it, expect } from "vitest";
import { filterSlashMenuItems } from "../SlashMenuItems";

describe("SlashMenuItems", () => {
  describe("filterSlashMenuItems", () => {
    const items = [
      { title: "Heading 1", aliases: ["h1", "biaoti"] },
      { title: "Image", aliases: ["tupian"] },
      { title: "Code Block", aliases: ["pre"] },
    ];

    it("should filter by title case-insensitive", () => {
      expect(filterSlashMenuItems(items, "heading")).toHaveLength(1);
      expect(filterSlashMenuItems(items, "HEADING")).toHaveLength(1);
      expect(filterSlashMenuItems(items, "code")).toHaveLength(1);
      expect(filterSlashMenuItems(items, "xyz")).toHaveLength(0);
    });

    it("should filter by alias", () => {
      expect(filterSlashMenuItems(items, "h1")).toHaveLength(1);
      expect(filterSlashMenuItems(items, "biaoti")).toHaveLength(1); // Pinyin check
      expect(filterSlashMenuItems(items, "tupian")).toHaveLength(1);
    });

    it("should return all on empty query", () => {
      // Logic: includes("") returns true
      expect(filterSlashMenuItems(items, "")).toHaveLength(3);
    });

    it("should handle partial matches", () => {
      expect(filterSlashMenuItems(items, "head")).toHaveLength(1);
      expect(filterSlashMenuItems(items, "biao")).toHaveLength(1);
    });
  });
});
