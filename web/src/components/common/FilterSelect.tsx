/**
 * FilterSelect.tsx
 * 专用于筛选场景的现代化Select组件
 */

import React from 'react'
import { Filter } from 'lucide-react'
import ModernSelect, { SelectOption } from './ModernSelect'

/** FilterSelect组件Props */
export interface FilterSelectProps {
  /** 当前选中值 */
  value: string
  /** 值变更回调 */
  onValueChange: (value: string) => void
  /** 选项列表 */
  options: SelectOption[]
  /** 筛选器标签 */
  label?: string
  /** 占位符文本 */
  placeholder?: string
  /** 是否显示筛选图标 */
  showIcon?: boolean
  /** 自定义样式类名 */
  className?: string
}

/**
 * FilterSelect 筛选器选择组件
 * - 专门用于各种筛选场景
 * - 带有筛选图标和标签
 * - 统一的筛选器外观
 */
const FilterSelect: React.FC<FilterSelectProps> = ({
  value,
  onValueChange,
  options,
  label,
  placeholder = "全部",
  showIcon = true,
  className = "",
}) => {
  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {showIcon && (
        <Filter size={16} className="text-neutral-500 flex-shrink-0" />
      )}
      {label && (
        <span className="text-sm text-neutral-600 whitespace-nowrap">
          {label}
        </span>
      )}
      <ModernSelect
        value={value}
        onValueChange={onValueChange}
        options={options}
        placeholder={placeholder}
        triggerClassName="min-w-[120px]"
      />
    </div>
  )
}

export default FilterSelect