/**
 * BlockNote 中文语言包
 *
 * 基于 @blocknote/core/locales 的内置中文支持
 * 可以在此基础上进行自定义覆盖
 */
import { zh } from "@blocknote/core/locales";

// 如果需要自定义某些翻译，可以在这里扩展
// 如果需要自定义某些翻译，可以在这里扩展
const customZh = {
  ...zh,
  // 示例：覆盖占位符
  // placeholders: {
  //   ...zh.placeholders,
  //   default: "输入 / 唤起菜单...",
  // },
};

export default customZh;
