/**
 * BlockNote 自定义 Schema
 *
 * 定义编辑器支持的块类型，包括标准 Markdown 块和 VitePress 扩展块
 */

import {
  BlockNoteSchema,
  defaultBlockSpecs,
  defaultInlineContentSpecs,
} from "@blocknote/core";
import {
  ContainerBlock,
  MathBlock,
  MermaidBlock,
  InlineMath,
  VueComponentBlock,
  IncludeBlock,
  ShikiCodeBlock,
  QuoteBlock,
} from "./blocks";

// 创建自定义 Schema
export const customSchema = BlockNoteSchema.create({
  blockSpecs: {
    // 保留标准块
    paragraph: defaultBlockSpecs.paragraph,
    heading: defaultBlockSpecs.heading,
    bulletListItem: defaultBlockSpecs.bulletListItem,
    numberedListItem: defaultBlockSpecs.numberedListItem,
    checkListItem: defaultBlockSpecs.checkListItem,
    codeBlock: defaultBlockSpecs.codeBlock,
    table: defaultBlockSpecs.table,
    image: defaultBlockSpecs.image,

    // E2.3：自定义块（createReactBlockSpec 返回函数，需调用）
    container: ContainerBlock(),
    math: MathBlock(),
    mermaid: MermaidBlock(),

    // E2.4：VitePress 特色语法块 (createReactBlockSpec 返回工厂函数，需调用)
    vueComponent: VueComponentBlock(),
    include: IncludeBlock(),
    shikiCode: ShikiCodeBlock(),
    quote: QuoteBlock(),
  },
  inlineContentSpecs: {
    // 保留默认内联内容
    ...defaultInlineContentSpecs,

    // E2.3：自定义内联内容（createReactInlineContentSpec 返回对象，不需调用）
    inlineMath: InlineMath,
  },
});

export type CustomSchema = typeof customSchema;
