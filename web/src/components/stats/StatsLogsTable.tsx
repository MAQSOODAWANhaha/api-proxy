import { LoadingSpinner } from '@/components/ui/loading'

import type { LogsPage, LogItem } from '@/types/stats'
import { Button } from '@/components/ui/button'
import { useTimezoneStore } from '@/store/timezone'

interface StatsLogsTableProps {
  logs: LogsPage | null
  loading?: boolean
  onPageChange: (page: number) => void
  hasFetched: boolean
}

const formatTimestamp = (timestamp: string, timezone: string) => {
  const date = new Date(timestamp)
  return {
    date: date.toLocaleDateString('zh-CN', { timeZone: timezone }),
    time: date.toLocaleTimeString('zh-CN', { timeZone: timezone, hour12: false }),
  }
}

const StatusPill = ({ item }: { item: LogItem }) => {
  const success = item.is_success
  const base =
    'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ring-1'
  const classes = success
    ? 'bg-emerald-50 text-emerald-600 ring-emerald-200'
    : 'bg-rose-50 text-rose-600 ring-rose-200'
  return <span className={`${base} ${classes}`}>{item.status_code ?? '--'}</span>
}

export function StatsLogsTable({ logs, loading, onPageChange, hasFetched }: StatsLogsTableProps) {
  const timezone = useTimezoneStore((state) => state.timezone)

  const page = logs?.page ?? 1
  const pageSize = logs?.page_size ?? 20
  const total = logs?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const rows: LogItem[] = logs?.items ?? []

  const handlePageChange = (nextPage: number) => {
    if (nextPage < 1 || nextPage > totalPages || nextPage === page) return
    onPageChange(nextPage)
  }

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white shadow-sm">
      <div className="flex flex-col gap-1 border-b border-neutral-200 px-6 py-5 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-base font-semibold text-neutral-900">请求记录</h2>
          <p className="text-xs text-neutral-500">最新调用记录，帮助定位问题与复盘请求表现</p>
        </div>
        <span className="text-xs text-neutral-400">时间显示时区：{timezone}</span>
      </div>

      <div className="relative overflow-x-auto">
        {loading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-white/70">
            <LoadingSpinner size="md" tone="muted" />
          </div>
        )}
        <table className="w-full text-sm">
          <thead className="bg-neutral-50 text-neutral-600">
            <tr>
              <th className="px-5 py-3 text-left font-medium">时间</th>
              <th className="px-5 py-3 text-left font-medium">请求信息</th>
              <th className="px-5 py-3 text-left font-medium">状态</th>
              <th className="px-5 py-3 text-left font-medium">模型</th>
              <th className="px-5 py-3 text-left font-medium">Token</th>
              <th className="px-5 py-3 text-left font-medium">费用</th>
              <th className="px-5 py-3 text-left font-medium">耗时</th>
              <th className="px-5 py-3 text-left font-medium">客户端</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-neutral-100">
            {!hasFetched ? (
              <tr>
                <td colSpan={8} className="px-5 py-10 text-center text-sm text-neutral-500">
                  请输入用户 API Key 并点击查询，日志将显示在此处。
                </td>
              </tr>
            ) : rows.length === 0 ? (
              <tr>
                <td colSpan={8} className="px-5 py-10 text-center text-sm text-neutral-500">
                  当前条件下暂无调用记录。
                </td>
              </tr>
            ) : (
              rows.map((item) => {
                const { date, time } = formatTimestamp(item.timestamp, timezone)
                const method = item.method ? item.method.toUpperCase() : '--'
                const costCurrency = item.cost_currency ?? 'USD'

                return (
                  <tr key={item.id} className="text-neutral-800 hover:bg-neutral-50">
                    <td className="px-5 py-3 align-top">
                      <div className="flex flex-col text-xs">
                        <span className="font-medium text-neutral-700">{date}</span>
                        <span className="font-mono text-neutral-500">{time}</span>
                      </div>
                    </td>
                    <td className="px-5 py-3 align-top">
                      <div className="space-y-1 text-xs text-neutral-600">
                        <div className="font-medium text-neutral-800">
                          {method} {item.path ?? '-'}
                        </div>
                        <div className="text-neutral-400">Request ID: {item.request_id}</div>
                      </div>
                    </td>
                    <td className="px-5 py-3 align-top">
                      <StatusPill item={item} />
                    </td>
                    <td className="px-5 py-3 align-top text-xs text-neutral-600">
                      {item.model ?? '-'}
                    </td>
                    <td className="px-5 py-3 align-top text-xs text-neutral-600">
                      <div className="font-medium text-neutral-800">
                        总计：{item.tokens_total.toLocaleString()}
                      </div>
                      <div className="text-neutral-400 space-y-0.5">
                        <div>输入：{item.tokens_prompt.toLocaleString()} | 输出：{item.tokens_completion.toLocaleString()}</div>
                        <div>缓存创建：{item.cache_create_tokens?.toLocaleString?.() ?? '0'} | 缓存读取：{item.cache_read_tokens?.toLocaleString?.() ?? '0'}</div>
                      </div>
                    </td>
                    <td className="px-5 py-3 align-top text-xs text-neutral-600">
                      {item.cost != null ? `${costCurrency} ${item.cost.toFixed(4)}` : '--'}
                    </td>
                    <td className="px-5 py-3 align-top text-xs text-neutral-600">
                      {item.duration_ms != null ? `${item.duration_ms} ms` : '--'}
                    </td>
                    <td className="px-5 py-3 align-top text-xs text-neutral-600">
                      <div className="space-y-1">
                        <div>{item.client_ip ?? '--'}</div>
                        <div className="text-neutral-400">{item.user_agent ?? '--'}</div>
                      </div>
                    </td>
                  </tr>
                )
              })
            )}
          </tbody>
        </table>
      </div>

      <div className="flex flex-col items-center justify-between gap-3 border-t border-neutral-200 px-5 py-4 text-sm text-neutral-500 sm:flex-row">
        <span>
          共 {total.toLocaleString()} 条记录 · 当前第 {page}/{totalPages} 页
        </span>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => handlePageChange(page - 1)} disabled={page <= 1 || loading}>
            上一页
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handlePageChange(page + 1)}
            disabled={page >= totalPages || loading}
          >
            下一页
          </Button>
        </div>
      </div>
    </div>
  )
}
