/**
 * QuoteBlock - Markdown 引用块
 *
 * 支持标准 Markdown blockquote (`> text`) 的原生编辑
 * 样式通过 CSS (.node-quote) 应用，支持嵌套子块边框延续
 */

import { createReactBlockSpec } from "@blocknote/react";

export const QuoteBlock = createReactBlockSpec(
  {
    type: "quote",
    propSchema: {
      groupId: {
        default: "default",
      },
      isFirstInGroup: {
        default: true,
      },
    },
    content: "inline",
  },
  {
    render: (props) => {
      const isFirstInGroup = props.block.props.isFirstInGroup;
      const siblingClass = isFirstInGroup ? "" : " quote-block-sibling";

      return (
        <div className={`quote-block w-full py-2 text-gray-600${siblingClass}`}>
          {/* 引用块自身的内联内容 */}
          <div
            className="inline-content quote-content"
            ref={props.contentRef}
          />
        </div>
      );
    },
  }
);
