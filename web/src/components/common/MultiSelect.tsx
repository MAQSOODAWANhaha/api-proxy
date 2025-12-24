/**
 * MultiSelect.tsx
 * 基于shadcn/ui的多选组件，支持搜索和多选功能
 */

import React, { useState, useCallback } from 'react'
import { Check, ChevronDown, Search, X } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'

/** 多选选项接口 */
export interface MultiSelectOption {
  value: string
  label: string
  disabled?: boolean
}

/** MultiSelect组件Props */
export interface MultiSelectProps {
  /** 当前选中的值数组 */
  value: string[]
  /** 值变更回调 */
  onValueChange: (value: string[]) => void
  /** 选项列表 */
  options: MultiSelectOption[]
  /** 占位符文本 */
  placeholder?: string
  /** 搜索占位符 */
  searchPlaceholder?: string
  /** 是否支持搜索 */
  searchable?: boolean
  /** 最大显示数量 */
  maxDisplay?: number
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义样式类名 */
  className?: string
}

/**
 * MultiSelect 多选组件
 * - 基于 shadcn/ui 设计系统
 * - 支持搜索过滤选项
 * - 支持键盘导航
 * - 自动适配主题
 */
const MultiSelect: React.FC<MultiSelectProps> = ({
  value,
  onValueChange,
  options,
  placeholder = "请选择...",
  searchPlaceholder = "搜索选项...",
  searchable = true,
  maxDisplay = 2,
  disabled = false,
  className = "",
}) => {
  const [isOpen, setIsOpen] = useState(false)
  const [searchTerm, setSearchTerm] = useState('')

  // 过滤选项
  const filteredOptions = options.filter(option =>
    option.label.toLowerCase().includes(searchTerm.toLowerCase()) ||
    option.value.toLowerCase().includes(searchTerm.toLowerCase())
  )

  // 切换选项选中状态
  const toggleOption = useCallback((optionValue: string) => {
    if (disabled) return
    
    const newValue = value.includes(optionValue)
      ? value.filter(v => v !== optionValue)
      : [...value, optionValue]
    
    onValueChange(newValue)
  }, [value, onValueChange, disabled])

  // 移除选中项
  const removeValue = useCallback((optionValue: string) => {
    onValueChange(value.filter(v => v !== optionValue))
  }, [value, onValueChange])

  // 清除所有选择
  const clearAll = useCallback(() => {
    onValueChange([])
    setSearchTerm('')
  }, [onValueChange])

  // 获取选中项的标签
  const getSelectedLabels = () => {
    return value.map(v => options.find(opt => opt.value === v)?.label || v)
  }

  const selectedLabels = getSelectedLabels()

  return (
    <div className={`relative ${className}`}>
      {/* 触发器 */}
      <div
        onClick={() => !disabled && setIsOpen(!isOpen)}
        className={`
          w-full min-h-[2.5rem] px-3 py-2 border border-input rounded-md 
          bg-background text-sm cursor-pointer transition-colors
          ${disabled ? 'opacity-50 cursor-not-allowed' : 'hover:bg-accent'}
          ${isOpen ? 'ring-2 ring-ring ring-offset-2' : ''}
          flex items-center justify-between gap-2
        `}
      >
        <div className="flex-1 flex items-center gap-2">
          {value.length === 0 ? (
            <span className="text-muted-foreground">{placeholder}</span>
          ) : (
            <div className="flex items-center gap-1 flex-wrap">
              {selectedLabels.slice(0, maxDisplay).map((label, index) => (
                <Badge 
                  key={value[index]} 
                  variant="secondary"
                  className="text-xs"
                >
                  {label}
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation()
                      removeValue(value[index])
                    }}
                    className="ml-1 hover:bg-muted rounded-full p-0.5"
                  >
                    <X size={10} />
                  </button>
                </Badge>
              ))}
              {value.length > maxDisplay && (
                <Badge variant="outline" className="text-xs">
                  +{value.length - maxDisplay}
                </Badge>
              )}
            </div>
          )}
        </div>
        <ChevronDown 
          size={16} 
          className={`text-muted-foreground transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </div>

      {/* 下拉内容 */}
      {isOpen && (
        <div className="absolute top-full left-0 right-0 mt-2 bg-popover border border-border rounded-md shadow-lg z-50 overflow-hidden">
          {/* 搜索框 */}
          {searchable && (
            <div className="p-3 border-b border-border">
              <div className="relative">
                <Search size={16} className="absolute left-3 top-1/2 transform -translate-y-1/2 text-muted-foreground" />
                <Input
                  type="text"
                  placeholder={searchPlaceholder}
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                  className="pl-10 h-8"
                />
              </div>
            </div>
          )}
          
          {/* 选项列表 */}
          <div className="max-h-48 overflow-y-auto">
            {filteredOptions.length > 0 ? (
              filteredOptions.map(option => {
                const isSelected = value.includes(option.value)
                return (
                  <div
                    key={option.value}
                    onClick={() => toggleOption(option.value)}
                    className={`
                      px-3 py-2.5 cursor-pointer transition-colors
                      ${option.disabled ? 'opacity-50 cursor-not-allowed' : 'hover:bg-accent'}
                      ${isSelected ? 'bg-accent' : ''}
                      flex items-center gap-3
                    `}
                  >
                    <div className={`
                      w-4 h-4 border-2 rounded flex items-center justify-center transition-all
                      ${isSelected 
                        ? 'bg-primary border-primary' 
                        : 'border-muted-foreground'
                      }
                    `}>
                      {isSelected && <Check size={12} className="text-primary-foreground" />}
                    </div>
                    <span className={`text-sm flex-1 ${isSelected ? 'font-medium' : ''}`}>
                      {option.label}
                    </span>
                  </div>
                )
              })
            ) : (
              <div className="px-3 py-4 text-center text-muted-foreground text-sm">
                没有找到匹配的选项
              </div>
            )}
          </div>
          
          {/* 底部操作 */}
          {value.length > 0 && (
            <div className="p-3 border-t border-border bg-muted/50">
              <button
                type="button"
                onClick={clearAll}
                className="text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                清除所有选择 ({value.length} 项)
              </button>
            </div>
          )}
        </div>
      )}
      
      {/* 点击外部关闭 */}
      {isOpen && (
        <div 
          className="fixed inset-0 z-40"
          onClick={() => setIsOpen(false)}
        />
      )}
    </div>
  )
}

export default MultiSelect
