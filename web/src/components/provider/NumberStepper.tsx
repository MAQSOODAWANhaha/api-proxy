/**
 * NumberStepper.tsx
 * 数字步进器：用于权重、请求/Token 限制等数值调整，支持最小值与步进值。
 */

import React from 'react'
import { Button } from '@/components/ui/button'
import { Minus, Plus } from 'lucide-react'

/** NumberStepper 组件属性 */
export interface NumberStepperProps {
  /** 当前值 */
  value: number
  /** 变更回调 */
  onChange: (v: number) => void
  /** 最小值（默认 0） */
  min?: number
  /** 步进（默认 1） */
  step?: number
  /** 禁用 */
  disabled?: boolean
  /** 额外类名 */
  className?: string
}

/**
 * NumberStepper 组件
 * - 提供 - / + 两端按钮和中间展示值
 */
const NumberStepper: React.FC<NumberStepperProps> = ({
  value,
  onChange,
  min = 0,
  step = 1,
  disabled,
  className,
}) => {
  /** 减少数值，保证不小于最小值 */
  const dec = () => {
    if (disabled) return
    const next = Math.max(min, value - step)
    onChange(next)
  }

  /** 增加数值 */
  const inc = () => {
    if (disabled) return
    onChange(value + step)
  }

  return (
    <div className={['flex items-center gap-2', className || ''].join(' ')}>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="bg-transparent h-8 w-8 p-0"
        onClick={dec}
        aria-label="减少"
        disabled={disabled || value <= min}
      >
        <Minus size={16} />
      </Button>
      <div
        className="flex h-8 w-12 items-center justify-center rounded-md border bg-background text-sm"
        aria-live="polite"
      >
        {value}
      </div>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="bg-transparent h-8 w-8 p-0"
        onClick={inc}
        aria-label="增加"
        disabled={disabled}
      >
        <Plus size={16} />
      </Button>
    </div>
  )
}

export default NumberStepper
