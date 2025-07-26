/**
 * 图表工具函数
 */

// 格式化数字
export function formatNumber(num: number): string {
  if (num >= 1000000000) {
    return (num / 1000000000).toFixed(1) + 'B'
  } else if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K'
  }
  return num.toLocaleString()
}

// 计算变化趋势
export function calculateChange(current: number, previous: number): {
  change: number
  changeType: 'increase' | 'decrease' | 'neutral'
} {
  if (previous === 0) {
    return {
      change: current > 0 ? 100 : 0,
      changeType: current > 0 ? 'increase' : 'neutral'
    }
  }

  const change = ((current - previous) / previous) * 100
  
  return {
    change: Math.abs(change),
    changeType: change > 0 ? 'increase' : change < 0 ? 'decrease' : 'neutral'
  }
}