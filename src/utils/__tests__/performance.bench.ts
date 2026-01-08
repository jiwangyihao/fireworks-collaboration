import { bench, describe } from "vitest";
import { markdownToBlocks } from "../markdown-to-blocks";
import { blocksToMarkdown } from "../blocks-to-markdown";

describe("转换器性能", () => {
  const smallDoc = "# 标题\n\n段落\n\n- 列表";
  const mediumDoc = smallDoc.repeat(100);
  const largeDoc = smallDoc.repeat(1000);

  bench("小文档 Markdown -> Blocks", () => {
    markdownToBlocks(smallDoc);
  });

  bench("中等文档 Markdown -> Blocks", () => {
    markdownToBlocks(mediumDoc);
  });

  bench(
    "大文档 Markdown -> Blocks",
    () => {
      markdownToBlocks(largeDoc);
    },
    { time: 5000 }
  );

  bench("往返转换一致性", async () => {
    const blocks = await markdownToBlocks(mediumDoc);
    const markdown = await blocksToMarkdown(blocks);
    await markdownToBlocks(markdown);
  });
});
