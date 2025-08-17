/**
 * ModernSelect.tsx
 * 基于shadcn/ui的现代化Select组件，替换原生HTML select
 */

import React from 'react'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

/** 选项数据接口 */
export interface SelectOption {
  value: string
  label: string
  disabled?: boolean
}

/** ModernSelect组件Props */
export interface ModernSelectProps {
  /** 当前选中值 */
  value: string
  /** 值变更回调 */
  onValueChange: (value: string) => void
  /** 选项列表 */
  options: SelectOption[]
  /** 占位符文本 */
  placeholder?: string
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义样式类名 */
  className?: string
  /** 触发器样式类名 */
  triggerClassName?: string
  /** 内容样式类名 */
  contentClassName?: string
}

/**
 * ModernSelect 现代化选择器组件
 * - 基于 shadcn/ui Select 组件
 * - 支持键盘导航和无障碍访问
 * - 自动适配主题（亮/暗模式）
 * - 支持自定义样式
 */
const ModernSelect: React.FC<ModernSelectProps> = ({
  value,
  onValueChange,
  options,
  placeholder = "请选择...",
  disabled = false,
  className = "",
  triggerClassName = "",
  contentClassName = "",
}) => {
  return (
    <div className={className}>
      <Select value={value} onValueChange={onValueChange} disabled={disabled}>
        <SelectTrigger 
          className={`h-9 text-sm ${triggerClassName}`}
          aria-label={placeholder}
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent className={contentClassName}>
          {options.map((option) => (
            <SelectItem
              key={option.value}
              value={option.value}
              disabled={option.disabled}
              className="text-sm cursor-pointer"
            >
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}

export default ModernSelect