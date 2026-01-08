import { useMemo } from 'react'
import { Pie, PieChart, Cell, Tooltip, ResponsiveContainer } from 'recharts'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import type { ModelShareItem } from '@/types/stats'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { LoadingSpinner } from '@/components/ui/loading'
import { useIsMobile } from '@/hooks/use-mobile'

interface StatsModelShareProps {
  data: {
    today: ModelShareItem[]
    total: ModelShareItem[]
  }
  loading?: boolean
  hasFetched: boolean
  scope: 'today' | 'total'
  onScopeChange?: (scope: 'today' | 'total') => void
}

const COLORS = ['hsl(var(--chart-1))', 'hsl(var(--chart-2))', 'hsl(var(--chart-3))', 'hsl(var(--chart-4))', 'hsl(var(--chart-5))']

const scopeLabel: Record<'today' | 'total', string> = {
  today: '今日',
  total: '累计',
}

export function StatsModelShare({ data, loading, hasFetched, scope, onScopeChange }: StatsModelShareProps) {
  const isMobile = useIsMobile()
  const current = useMemo(
    () =>
      (data[scope] ?? []).map((item, index) => ({
        ...item,
        color: COLORS[index % COLORS.length],
        percentage: Number(item.percentage.toFixed(2)),
        cost: item.cost ?? 0,
      })),
    [data, scope]
  )

  return (
    <Card className="rounded-2xl border border-neutral-200 bg-white">
      <CardHeader className="pb-4">
        <div className="flex w-full flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <CardTitle className="text-lg font-semibold text-neutral-800">模型占比</CardTitle>
            <ToggleGroup
              type="single"
              value={scope}
              onValueChange={(value) => {
                if (value === 'today' || value === 'total') {
                  onScopeChange?.(value)
                }
              }}
              className="rounded-lg border border-neutral-200 bg-neutral-50 p-1 text-xs"
            >
              {(['today', 'total'] as const).map((option) => (
                <ToggleGroupItem
                  key={option}
                  value={option}
                  className="px-3 py-1 text-neutral-700 data-[state=on]:bg-white data-[state=on]:text-violet-700"
                >
                  {scopeLabel[option]}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </div>
          <CardDescription className="leading-relaxed">模型请求占比</CardDescription>
        </div>
      </CardHeader>
      <CardContent className="pt-0">
        {loading ? (
          <div className="flex h-[280px] items-center justify-center text-muted-foreground">
            <LoadingSpinner size="md" tone="muted" />
          </div>
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
            <div className="h-[240px] sm:h-[280px]">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={current}
                    dataKey="percentage"
                    nameKey="model"
                    innerRadius={isMobile ? 52 : 60}
                    outerRadius={isMobile ? 92 : 110}
                    paddingAngle={4}
                  >
                    {current.map((entry) => (
                      <Cell key={entry.model} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    formatter={(value: number, _name, payload) => {
                      const item = payload.payload as ModelShareItem & { color: string; cost?: number }
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

            <div className="max-h-[240px] space-y-3 overflow-auto pr-1 sm:max-h-[280px]">
              {current.map((item) => (
                <div
                  key={`${scope}-${item.model}`}
                  className="rounded-lg border border-border/60 bg-muted/40 px-3 py-2 text-sm"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex min-w-0 items-start gap-2">
                      <span
                        className="mt-1 h-2 w-2 shrink-0 rounded-full"
                        style={{ backgroundColor: item.color }}
                      />
                      <div className="min-w-0">
                        <div className="truncate font-medium text-foreground">{item.model}</div>
                        <div className="mt-1 grid grid-cols-3 gap-x-3 gap-y-1 text-[11px] text-muted-foreground sm:grid-cols-1 sm:text-xs">
                          <div className="flex items-baseline gap-1">
                            <span>请求</span>
                            <span className="font-mono tabular-nums">{item.requests.toLocaleString()}</span>
                          </div>
                          <div className="flex items-baseline gap-1">
                            <span>Token</span>
                            <span className="font-mono tabular-nums">{item.tokens.toLocaleString()}</span>
                          </div>
                          <div className="flex items-baseline gap-1">
                            <span>费用</span>
                            <span className="font-mono tabular-nums">
                              ${item.cost.toFixed(2)}
                            </span>
                          </div>
                        </div>
                      </div>
                    </div>
                    <div className="shrink-0 text-right">
                      <div className="font-mono text-sm font-medium tabular-nums text-foreground">
                        {item.percentage.toFixed(1)}%
                      </div>
                      <div className="text-[11px] text-muted-foreground">{scopeLabel[scope]}</div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
