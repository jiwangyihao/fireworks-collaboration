/**
 * Markdown 转换器统一导出入口
 *
 * 提供 Markdown ↔ Block 双向转换的公共 API。
 */

// 类型导出
export * from "../types/block";
export * from "../types/document";

// Markdown → Block
export {
  markdownToBlocks,
  parseMarkdownDocument,
  extractMarkdownFrontmatter,
} from "./markdown-to-blocks";

// Block → Markdown
export {
  blocksToMarkdown,
  documentToMarkdown,
  singleBlockToMarkdown,
} from "./blocks-to-markdown";
