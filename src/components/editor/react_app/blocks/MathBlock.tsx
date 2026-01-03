// @ts-nocheck
/**
 * MathBlock - MathLive 公式块
 *
 * 使用 MathLive <math-field> 提供所见即所得的数学公式编辑体验 (Block Level)
 */

import { createReactBlockSpec } from "@blocknote/react";
import { useRef, useEffect } from "react";
import "mathlive";
import { MathfieldElement } from "mathlive";

// 设置全局字体路径和语言环境
try {
  MathfieldElement.fontsDirectory = "/fonts";
  MathfieldElement.locale = "zh-cn";
} catch (e) {
  // Ignore
}

declare global {
  namespace JSX {
    interface IntrinsicElements {
      "math-field": any;
    }
  }
}

export const MathBlock = createReactBlockSpec(
  {
    type: "math",
    propSchema: {
      formula: {
        default: "",
      },
    },
    content: "none",
  },
  {
    render: (props) => {
      const formula = props.block.props.formula as string;
      const ref = useRef<MathfieldElement>(null);

      // 实时更新 Block 属性
      const handleInput = (e: React.SyntheticEvent<MathfieldElement>) => {
        const val = (e.target as MathfieldElement).value;
        props.editor.updateBlock(props.block, {
          props: { formula: val },
        });
      };

      useEffect(() => {
        // locale is handled globally
      }, []);

      return (
        <div className="w-full my-2 relative" data-math-block>
          <math-field
            ref={ref}
            // 使用 CSS 类控制样式 (见 style.css)
            // block-math 样式让它居中显示
            class="w-full p-2 text-lg text-center"
            style={{
              display: "block",
              width: "100%",
            }}
            onInput={handleInput}
            onMouseDown={(e: MouseEvent) => e.stopPropagation()}
            onClick={(e: MouseEvent) => e.stopPropagation()}
          >
            {formula}
          </math-field>
        </div>
      );
    },
  }
);
