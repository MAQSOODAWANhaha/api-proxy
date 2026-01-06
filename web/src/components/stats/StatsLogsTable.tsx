import { LoadingSpinner } from '@/components/ui/loading'
import DataTableShell from '@/components/common/DataTableShell'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

import type { LogsPage, LogItem } from '@/types/stats'
import { useTimezoneStore } from '@/store/timezone'
import Pagination from '@/components/common/Pagination'

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
  const classes = success ? 'table-status-success' : 'table-status-danger'
  return <span className={classes}>{item.status_code ?? '--'}</span>
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
    <DataTableShell>
      <div className="flex flex-col gap-1 border-b border-neutral-200 px-6 py-5 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-base font-semibold text-neutral-900">请求记录</h2>
          <p className="text-xs text-neutral-500">最新调用记录，帮助定位问题与复盘请求表现</p>
        </div>
        <span className="text-xs text-neutral-400">时间显示时区：{timezone}</span>
      </div>

      <div className="relative">
        {loading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-white/70">
            <LoadingSpinner size="md" tone="muted" />
          </div>
        )}
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>时间</TableHead>
              <TableHead>请求信息</TableHead>
              <TableHead>状态</TableHead>
              <TableHead>模型</TableHead>
              <TableHead>Token</TableHead>
              <TableHead>费用</TableHead>
              <TableHead>耗时</TableHead>
              <TableHead>客户端</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {!hasFetched ? (
              <TableRow>
                <TableCell colSpan={8} className="py-10 text-center text-sm text-neutral-500">
                  请输入用户 API Key 并点击查询，日志将显示在此处。
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="py-10 text-center text-sm text-neutral-500">
                  当前条件下暂无调用记录。
                </TableCell>
              </TableRow>
            ) : (
              rows.map((item) => {
                const { date, time } = formatTimestamp(item.timestamp, timezone)
                const method = item.method ? item.method.toUpperCase() : '--'
                const costCurrency = item.cost_currency ?? 'USD'

                return (
                  <TableRow key={item.id}>
                    <TableCell className="align-top">
                      <div className="flex flex-col text-xs">
                        <span className="table-subtext">{date}</span>
                        <span className="table-subtext font-mono">{time}</span>
                      </div>
                    </TableCell>
                    <TableCell className="align-top">
                      <div className="space-y-1 text-xs">
                        <div className="font-medium text-neutral-800">
                          {method} {item.path ?? '-'}
                        </div>
                        <div className="table-subtext">Request ID: {item.request_id}</div>
                      </div>
                    </TableCell>
                    <TableCell className="align-top">
                      <StatusPill item={item} />
                    </TableCell>
                    <TableCell className="align-top">
                      <span className="table-subtext">{item.model ?? '-'}</span>
                    </TableCell>
                    <TableCell className="align-top">
                      <div className="text-xs">
                        <div className="font-medium text-neutral-800">
                          总计：{item.tokens_total.toLocaleString()}
                        </div>
                        <div className="table-subtext space-y-0.5">
                          <div>输入：{item.tokens_prompt.toLocaleString()} | 输出：{item.tokens_completion.toLocaleString()}</div>
                          <div>缓存创建：{item.cache_create_tokens?.toLocaleString?.() ?? '0'} | 缓存读取：{item.cache_read_tokens?.toLocaleString?.() ?? '0'}</div>
                        </div>
                      </div>
                    </TableCell>
                    <TableCell className="align-top">
                      <span className="table-subtext">
                        {item.cost != null ? `${costCurrency} ${item.cost.toFixed(4)}` : '--'}
                      </span>
                    </TableCell>
                    <TableCell className="align-top">
                      <span className="table-subtext">
                        {item.duration_ms != null ? `${item.duration_ms} ms` : '--'}
                      </span>
                    </TableCell>
                    <TableCell className="align-top">
                      <div className="table-subtext space-y-1">
                        <div>{item.client_ip ?? '--'}</div>
                        <div>{item.user_agent ?? '--'}</div>
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>

      <div className="border-t border-neutral-200 px-5 py-4">
        <Pagination
          total={total}
          page={page}
          pageSize={pageSize}
          onPageChange={handlePageChange}
          showSizeChanger={false}
          className="text-neutral-500"
        />
      </div>
    </DataTableShell>
  )
}
