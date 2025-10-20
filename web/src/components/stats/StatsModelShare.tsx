import { useMemo, useState } from 'react'
import { Pie, PieChart, Cell, Tooltip, ResponsiveContainer } from 'recharts'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import type { ModelShareItem } from '@/types/stats'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'

interface StatsModelShareProps {
  data: {
    today: ModelShareItem[]
    total: ModelShareItem[]
  }
  loading?: boolean
  hasFetched: boolean
}

const COLORS = ['hsl(var(--chart-1))', 'hsl(var(--chart-2))', 'hsl(var(--chart-3))', 'hsl(var(--chart-4))', 'hsl(var(--chart-5))']

export function StatsModelShare({ data, loading, hasFetched }: StatsModelShareProps) {
  const [scope, setScope] = useState<'today' | 'total'>('today')
  const current = useMemo(
    () =>
      (data[scope] ?? []).map((item, index) => ({
        ...item,
        color: COLORS[index % COLORS.length],
        percentage: Number(item.percentage.toFixed(2)),
      })),
    [data, scope]
  )

  return (
    <Card className="shadow-sm">
      <CardHeader className="pb-4">
        <div className="flex w-full flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <CardTitle className="text-lg font-semibold">模型占比</CardTitle>
            <Select value={scope} onValueChange={(value) => setScope(value as 'today' | 'total')}>
              <SelectTrigger className="w-[140px]">
                <SelectValue placeholder="今日" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="today">今日</SelectItem>
                <SelectItem value="total">总计</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <CardDescription className="leading-relaxed">
            模型请求占比
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {loading ? (
          <div className="flex h-[280px] items-center justify-center text-muted-foreground">加载中...</div>
        ) : !hasFetched ? (
          <div className="flex h-[280px] items-center justify-center text-sm text-muted-foreground">
            查询完成后将显示模型占比。
          </div>
        ) : current.length === 0 ? (
          <div className="flex h-[280px] items-center justify-center text-sm text-muted-foreground">
            当前条件下暂无模型数据。
          </div>
        ) : (
          <div className="mt-4 grid gap-6 md:grid-cols-[1fr_1fr]">
            <div className="h-[280px]">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={current}
                    dataKey="percentage"
                    nameKey="model"
                    innerRadius={60}
                    outerRadius={100}
                    paddingAngle={4}
                  >
                    {current.map((entry) => (
                      <Cell key={entry.model} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    formatter={(value: number, _name, payload) => {
                      const item = payload.payload as ModelShareItem & { color: string }
                      return [`${value.toFixed(1)}%`, item.model]
                    }}
                    contentStyle={{
                      borderRadius: 12,
                      border: '1px solid var(--border)',
                      backgroundColor: 'var(--card)',
                      color: 'var(--foreground)',
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            </div>

            <div className="space-y-3">
              {current.slice(0, 6).map((item) => (
                <div
                  key={`${scope}-${item.model}`}
                  className="flex items-center justify-between rounded-lg border border-border/60 bg-muted/40 px-3 py-2 text-sm"
                >
                  <div className="flex items-center gap-2">
                    <span className="h-2 w-2 rounded-full" style={{ backgroundColor: item.color }} />
                    <div className="flex flex-col">
                      <span className="font-medium text-foreground">{item.model}</span>
                      <span className="text-xs text-muted-foreground">
                        请求 {item.requests.toLocaleString()} · Token {item.tokens.toLocaleString()} · 费用 ${item.cost.toFixed(2)}
                      </span>
                    </div>
                  </div>
                  <span className="text-xs text-muted-foreground">{item.percentage.toFixed(1)}%</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
