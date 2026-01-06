import { MathfieldElement } from "mathlive";
import "mathlive";

// 标记是否已初始化
let isInitialized = false;

/**
 * 初始化 MathLive配置 (全局只需要一次)
 */
export function initMathLive() {
  if (isInitialized) return;

  try {
    MathfieldElement.fontsDirectory = "/fonts";
    MathfieldElement.locale = "zh-cn";

    // 自定义声音路径或其他全局设置
    // MathfieldElement.soundsDirectory = "/sounds";

    isInitialized = true;
  } catch (e) {
    console.warn("Failed to set MathLive fonts directory or locale", e);
  }
}

/**
 * React JSX 类型声明
 */
declare global {
  namespace JSX {
    interface IntrinsicElements {
      "math-field": any;
    }
  }
}
