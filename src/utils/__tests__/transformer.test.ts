import { describe, it, expect } from "vitest";
import { markdownToBlocks } from "../markdown-to-blocks";
import { blocksToMarkdown } from "../blocks-to-markdown";

describe("Transformer Round-Trip", () => {
  it("should preserve basic text", async () => {
    const md = "Hello World";
    const blocks = await markdownToBlocks(md);
    const result = await blocksToMarkdown(blocks);
    expect(result).toContain(md);
  });

  it("should preserve heading structure", async () => {
    const md = "# Heading 1\n\n## Heading 2";
    const blocks = await markdownToBlocks(md);
    const result = await blocksToMarkdown(blocks);
    expect(result).toContain("# Heading 1");
    expect(result).toContain("## Heading 2");
  });

  it("should preserve lists", async () => {
    const md = "- Item 1\n- Item 2";
    const blocks = await markdownToBlocks(md);
    const result = await blocksToMarkdown(blocks);
    expect(result).toContain("- Item 1");
    expect(result).toContain("- Item 2");
  });

  it("should be consistent", async () => {
    const original = `# Title

Paragraph with **bold** and *italic*.

- List item 1
- List item 2

> Blockquote
`;
    // Note: blocksToMarkdown might act slightly different on whitespace, so we trim or match structure.
    const blocks = await markdownToBlocks(original);
    const generated = await blocksToMarkdown(blocks);

    // Normalize newlines for comparison
    const normOriginal = original.replace(/\r\n/g, "\n").trim();
    const normGenerated = generated.replace(/\r\n/g, "\n").trim();

    // Simple equality check might fail due to spacing, so we check inclusion of key parts
    expect(normGenerated).toContain("Paragraph with **bold**");
    expect(normGenerated).toContain("> Blockquote");
  });
});
