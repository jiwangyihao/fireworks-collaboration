import { Extension, InputRule } from "@tiptap/core";

export const BlockInputRules = Extension.create({
  name: "blockInputRules",

  addInputRules() {
    return [
      // Block Math Input Rule: $$ -> Math Block
      new InputRule({
        find: /^\$\$ $/,
        handler: ({ state, range }) => {
          const { tr } = state;
          const { from } = range;
          const $from = state.doc.resolve(from);

          // 获取当前块的范围（整个段落）
          // $from.before() 应该是 paragraph 的起始位置
          const start = $from.before();
          const end = $from.after();

          // 确保我们在 TextBlock 中
          if (!$from.parent.isTextblock) return;

          const mathNode = this.editor.schema.nodes.math?.create();

          if (mathNode) {
            // 替换整个 Block
            tr.replaceWith(start, end, mathNode);
          }
        },
      }),

      // Mermaid Input Rule: ```mermaid -> Mermaid Block
      new InputRule({
        find: /^```mermaid $/,
        handler: ({ state, range }) => {
          const { tr } = state;
          const { from } = range;
          const $from = state.doc.resolve(from);

          const start = $from.before();
          const end = $from.after();

          if (!$from.parent.isTextblock) return;

          const mermaidNode = this.editor.schema.nodes.mermaid?.create();

          if (mermaidNode) {
            tr.replaceWith(start, end, mermaidNode);
          }
        },
      }),

      // Inline Math Input Rule: $formula$
      new InputRule({
        find: /\$([^$]+)\$$/,
        handler: ({ state, range, match }) => {
          const { tr } = state;
          const { from, to } = range;
          const formula = match[1];

          if (formula && formula.trim().length > 0) {
            const inlineMathNode = this.editor.schema.nodes.inlineMath?.create({
              formula,
            });

            if (inlineMathNode) {
              tr.replaceWith(from, to, inlineMathNode);
            }
          }
        },
      }),

      // Standard Markdown Link: [text](url)
      new InputRule({
        find: /\[(.+?)\]\((.+?)\)$/,
        handler: ({ state, range, match }) => {
          const { tr } = state;
          const { from, to } = range;
          const text = match[1];
          const href = match[2];

          const linkMark = this.editor.schema.marks.link;
          if (linkMark) {
            const textNode = this.editor.schema.text(text, [
              linkMark.create({ href }),
            ]);
            tr.replaceWith(from, to, textNode);
          }
        },
      }),

      // Quote Block Input Rule: > at line start -> Quote Block
      new InputRule({
        find: /^> $/,
        handler: ({ state, range }) => {
          const { tr } = state;
          const { from } = range;
          const $from = state.doc.resolve(from);

          const start = $from.before();
          const end = $from.after();

          if (!$from.parent.isTextblock) return;

          const quoteNode = this.editor.schema.nodes.quote?.create();

          if (quoteNode) {
            tr.replaceWith(start, end, quoteNode);
            // 将光标定位到新创建的 quote 块内部
            // 使用 TextSelection.near 通过类型断言
            const TextSelection = state.selection.constructor as any;
            if (TextSelection.near) {
              tr.setSelection(TextSelection.near(tr.doc.resolve(start + 1)));
            }
          }
        },
      }),
    ];
  },
});
