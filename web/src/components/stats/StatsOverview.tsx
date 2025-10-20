import { Loader2 } from 'lucide-react'

import { SummaryMetric } from '@/types/stats'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

interface StatsOverviewProps {
  metrics: SummaryMetric[]
  loading?: boolean
  hasFetched: boolean
}

const formatValue = (value: number, unit: SummaryMetric['unit']) => {
  switch (unit) {
    case 'usd':
      return `$${value.toFixed(2)}`
    case 'token':
      if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
      if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
      return value.toLocaleString()
    default:
      return value.toLocaleString()
  }
}

export function StatsOverview({ metrics, loading, hasFetched }: StatsOverviewProps) {
  if (loading && !hasFetched) {
    return (
      <div className="grid gap-4 md:grid-cols-3">
        {[1, 2, 3].map((key) => (
          <Card key={key} className="border-dashed">
            <CardContent className="flex h-32 items-center justify-center text-muted-foreground">
              <Loader2 className="h-5 w-5 animate-spin" />
            </CardContent>
          </Card>
        ))}
      </div>
    )
  }

  if (!hasFetched) {
    return (
      <Card className="border-dashed">
        <CardContent className="flex h-32 flex-col items-center justify-center text-center text-muted-foreground">
          <p className="text-sm">请输入有效的用户 API Key并点击（查询统计）以加载数据。</p>
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="grid gap-4 md:grid-cols-3">
      {metrics.map((metric) => {
        const deltaPositive = metric.delta >= 0
        const magnitude = Math.abs(metric.delta).toFixed(1)

        return (
          <Card key={metric.id} className="shadow-sm">
            <CardHeader className="flex-row items-center justify-between space-y-0">
              <CardTitle className="text-sm font-medium text-muted-foreground">
                {metric.label}
              </CardTitle>
              <Badge
                variant="outline"
                className={deltaPositive ? 'border-emerald-200 text-emerald-600' : 'border-rose-200 text-rose-600'}
              >
                {deltaPositive ? '+' : '-'}{magnitude}%
              </Badge>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="text-3xl font-semibold text-foreground">
                {formatValue(metric.today, metric.unit)}
              </div>
              <p className="text-xs text-muted-foreground">
                累计 {formatValue(metric.total, metric.unit)}
              </p>
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}
