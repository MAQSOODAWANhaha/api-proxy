/**
 * StatCard.tsx
 * 今日概览统计卡片，贴近第一张图的小卡片风格。
 */
import React from 'react'
import { Card, CardContent } from './ui/card'

/** 统计卡片属性 */
export interface StatCardProps {
  icon: React.ReactNode
  value: string
  title: string
  delta?: string
  bg?: string
}

/** 统计卡片 */
export default function StatCard({ icon, value, title, delta, bg = 'bg-pink-100' }: StatCardProps) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center gap-3">
          <div className={`flex h-12 w-12 items-center justify-center rounded-lg ${bg}`}>{icon}</div>
          <div className="flex flex-col">
            <div className="text-xl font-bold">{value}</div>
            <div className="text-xs text-muted-foreground">{title}</div>
            {delta && <div className="mt-1 text-xs text-emerald-600 dark:text-emerald-400">{delta}</div>}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
