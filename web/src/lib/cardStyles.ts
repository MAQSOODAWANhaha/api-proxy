/**
 * 统一卡片样式工具类
 * 为整个应用提供一致的卡片设计标准
 */

// 基础卡片样式
const cardBase = "bg-white border border-neutral-200"

// 卡片变体样式
export const cardVariants = {
  // 标准卡片 - 用于主要内容区域
  standard: `${cardBase} rounded-2xl p-6`,
  
  // 紧凑卡片 - 用于列表项和较小内容
  compact: `${cardBase} rounded-xl p-4`,
  
  // 超紧凑卡片 - 用于统计项和简短信息
  minimal: `${cardBase} rounded-xl p-3`,
  
  // 无内边距卡片 - 用于表格容器等
  container: `${cardBase} rounded-xl`,
  
  // 统计卡片 - 用于KPI展示
  stat: `${cardBase} rounded-xl p-4`,
}

// 内边距工具类
export const cardPadding = {
  none: '',
  small: 'p-3',
  medium: 'p-4', 
  large: 'p-6',
  xl: 'p-8',
}

// 圆角工具类
export const cardRadius = {
  default: 'rounded-xl',
  large: 'rounded-2xl',
  small: 'rounded-lg',
}

// 阴影工具类  
export const cardShadow = {
  none: '',
  default: '',
  strong: '',
  subtle: '',
}

// 组合工具函数
export const createCardClass = (
  variant: keyof typeof cardVariants = 'standard',
  extraClasses?: string
) => {
  return `${cardVariants[variant]} ${extraClasses || ''}`.trim()
}

// 自定义组合函数
export const customCard = (
  radius: keyof typeof cardRadius = 'default',
  padding: keyof typeof cardPadding = 'medium',
  shadow: keyof typeof cardShadow = 'default',
  extraClasses?: string
) => {
  return `${cardBase} ${cardRadius[radius]} ${cardPadding[padding]} ${cardShadow[shadow]} ${extraClasses || ''}`.trim()
}
