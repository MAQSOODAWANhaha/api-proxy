/**
 * ModelPie.tsx
 * 模型使用占比：GPT/Claude/Gemini 等分布。
 */

import React from 'react'
import { PieChart, Pie, Cell, ResponsiveContainer, Legend, Tooltip } from 'recharts'
import { TitledCard } from '@/components/common/UnifiedCard'

/** 模型占比数据结构 */
export interface ModelSlice {
  name: string
  value: number
}

/** 组件 props */
export interface ModelPieProps {
  /** 数据集合 */
  data: ModelSlice[]
}

/** 紫色系 + 对比色 */
const COLORS = ['#6D5BD0', '#8757E8', '#10b981', '#f59e0b', '#ef4444']

/**
 * ModelPie
 * - 白卡 + 细边样式
 * - 悬停 tooltip 展示具体数值
 */
const ModelPie: React.FC<ModelPieProps> = ({ data }) => {
  return (
    <TitledCard
      variant="compact"
      title="模型使用占比"
      headerClassName="pb-2"
      contentClassName="pt-2"
    >
      <div className="h-72">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart>
            <Pie
              data={data}
              dataKey="value"
              nameKey="name"
              innerRadius={60}
              outerRadius={100}
              paddingAngle={4}
              stroke="#fff"
              strokeWidth={2}
            >
              {data.map((_, index) => (
                <Cell key={`slice-${index}`} fill={COLORS[index % COLORS.length]} />
              ))}
            </Pie>
            <Legend />
            <Tooltip formatter={(v: any) => (typeof v === 'number' ? v.toLocaleString() : v)} />
          </PieChart>
        </ResponsiveContainer>
      </div>
    </TitledCard>
  )
}

export default ModelPie
