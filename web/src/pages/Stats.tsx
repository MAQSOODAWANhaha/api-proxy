/**
 * Stats.tsx
 * 统计分析摘要页面（示例图表）。移除页面内部大标题，改由 Topbar 展示。
 */
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { ResponsiveContainer, LineChart, Line, CartesianGrid, XAxis, YAxis, Tooltip } from 'recharts'

/** 模拟图表数据 */
const data = Array.from({ length: 12 }).map((_, i) => ({
  m: i + 1,
  value: Math.round(40 + Math.random() * 60),
}))

/** 统计分析页面 */
export default function StatsPage() {
  return (
    <div className="space-y-4">
      <Card className="shadow-sm">
        <CardHeader>
          <CardTitle>年度趋势</CardTitle>
        </CardHeader>
        <CardContent className="h-80">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={data}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis dataKey="m" />
              <YAxis />
              <Tooltip />
              <Line type="monotone" dataKey="value" stroke="#8b5cf6" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </CardContent>
      </Card>
    </div>
  )
}
