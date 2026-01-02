/**
 * BlockNote 自定义 Schema
 *
 * 定义编辑器支持的块类型，包括标准 Markdown 块和 VitePress 扩展块
 */

import { BlockNoteSchema, defaultBlockSpecs } from "@blocknote/core";

// 创建自定义 Schema
// 第一阶段：只使用默认块，后续 E2.3/E2.4 添加自定义块
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

    // TODO E2.3：新增自定义块
    // container: ContainerBlock,
    // math: MathBlock,
    // mermaid: MermaidBlock,

    // TODO E2.4：VitePress 特色语法块
    // vueComponent: VueComponentBlock,
    // include: IncludeBlock,
  },
});

export type CustomSchema = typeof customSchema;
