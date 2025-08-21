/**
 * ErrorBreakdown.tsx
 * 错误类型分布：4xx/5xx 等，柱状图呈现。
 */

import React from 'react'
import {
  ResponsiveContainer,
  BarChart,
  XAxis,
  YAxis,
  Tooltip as ReTooltip,
  Bar,
  CartesianGrid,
} from 'recharts'
import { TitledCard } from '@/components/common/UnifiedCard'

/** 错误分布数据结构 */
export interface ErrorBucket {
  name: string
  count: number
}

/** 组件 props */
export interface ErrorBreakdownProps {
  data: ErrorBucket[]
}

/**
 * ErrorBreakdown
 * - 紫色渐变柱，维持全局风格
 */
const ErrorBreakdown: React.FC<ErrorBreakdownProps> = ({ data }) => {
  return (
    <TitledCard
      variant="compact"
      title="错误类型分布"
      headerClassName="pb-2"
      contentClassName="pt-2"
    >
      <div className="h-72">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} margin={{ top: 10, right: 16, bottom: 0, left: 0 }}>
            <CartesianGrid stroke="#eee" vertical={false} />
            <XAxis dataKey="name" tick={{ fill: '#6b7280', fontSize: 12 }} />
            <YAxis tick={{ fill: '#6b7280', fontSize: 12 }} />
            <ReTooltip formatter={(v: any) => (typeof v === 'number' ? v.toLocaleString() : v)} />
            <defs>
              <linearGradient id="errGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#8757E8" />
                <stop offset="100%" stopColor="#6D5BD0" />
              </linearGradient>
            </defs>
            <Bar dataKey="count" fill="url(#errGradient)" radius={[6, 6, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </TitledCard>
  )
}

export default ErrorBreakdown
