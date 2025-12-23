/**
 * 格式化工具函数
 */

/**
 * 格式化数字，大于1000时显示为 k 格式
 * @param num 要格式化的数字
 * @returns 格式化后的字符串
 * @example formatNumber(1500) // "1.5k"
 */
export function formatNumber(num: number): string {
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + "k";
  }
  return num.toString();
}

/**
 * 将日期字符串转换为相对时间描述
 * @param dateStr 日期字符串
 * @returns 相对时间描述
 * @example relativeTime("2024-12-20") // "3 天前"
 */
export function relativeTime(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));

  if (days === 0) return "今天";
  if (days === 1) return "昨天";
  if (days < 7) return `${days} 天前`;
  if (days < 30) return `${Math.floor(days / 7)} 周前`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前`;
  return `${Math.floor(days / 365)} 年前`;
}
