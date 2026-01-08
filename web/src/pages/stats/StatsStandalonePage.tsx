import { useEffect, useState } from 'react'
import { RefreshCw, RotateCcw, Search } from 'lucide-react'

import { StatsOverview } from '@/components/stats/StatsOverview'
import { StatsTrendChart } from '@/components/stats/StatsTrendChart'
import { StatsModelShare } from '@/components/stats/StatsModelShare'
import { StatsLogsTable } from '@/components/stats/StatsLogsTable'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { useStatsStore, type Timeframe } from '@/store/stats'
import { useTimezoneStore } from '@/store/timezone'

const TREND_OPTIONS: Timeframe[] = ['7d', '30d']

export default function StatsStandalonePage() {
  const {
    fetch,
    fetchTrendOnly,
    fetchModelShareOnly,
    summary,
    trend,
    modelShare,
    logs,
    loading,
    trendLoading,
    modelShareLoading,
    error,
    filters,
    setFilters,
    hasFetched,
    clear,
  } = useStatsStore()

  const timezoneStore = useTimezoneStore()
  const [apiKeyInput, setApiKeyInput] = useState(filters.userServiceKey)
  const [trendTimeframe, setTrendTimeframe] = useState<Timeframe>('7d')
  const [modelScope, setModelScope] = useState<'today' | 'total'>('today')

  useEffect(() => {
    if (!timezoneStore.isInitialized) {
      timezoneStore.detectTimezone()
    }
  }, [timezoneStore])

  useEffect(() => {
    if (filters.userServiceKey && filters.userServiceKey !== apiKeyInput) {
      setApiKeyInput(filters.userServiceKey)
    }
    if (filters.timeframe && TREND_OPTIONS.includes(filters.timeframe)) {
      setTrendTimeframe(filters.timeframe)
    }
  }, [filters.userServiceKey, filters.timeframe, apiKeyInput])

  const hasServiceKey = filters.userServiceKey.trim().length > 0
  const pageBusy = loading || trendLoading || modelShareLoading
  const canSubmit = apiKeyInput.trim().length > 0 && !pageBusy

  const handleSubmit = () => {
    const key = apiKeyInput.trim()
    if (!key) return

    const rangePreset = trendTimeframe === '30d' ? '30d' : '7d'
    setModelScope('today')
    setFilters({
      userServiceKey: key,
      page: 1,
      rangePreset,
      timeframe: trendTimeframe,
      includeToday: true,
      pageSize: filters.pageSize,
      search: filters.search,
    })
    void fetch({
      userServiceKey: key,
      page: 1,
      rangePreset,
      timeframe: trendTimeframe,
      includeToday: true,
    })
  }

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="mx-auto flex min-h-screen w-full max-w-7xl flex-col gap-8 px-4 pb-12 pt-8 sm:gap-10 sm:px-8 sm:pb-16 sm:pt-12 lg:px-10">
        <header className="space-y-3 text-center">
          <h1 className="text-2xl font-semibold tracking-tight text-neutral-900 sm:text-3xl">
            用户 API Key 使用统计
          </h1>
          <p className="mx-auto max-w-2xl text-sm leading-relaxed text-neutral-600">
            在此查看指定用户服务密钥的请求趋势、模型占比以及最新调用日志。
          </p>
        </header>

        <Card className="mx-auto w-full max-w-7xl rounded-2xl border border-neutral-200 bg-white">
          <CardContent className="space-y-5 p-5 sm:p-8">
            <div className="space-y-2">
              <label className="block text-sm font-medium text-neutral-600" htmlFor="user-service-key">
                用户 API Key
              </label>
              <Input
                id="user-service-key"
                placeholder="请输入用户 API Key（例如：sk-usr-xxxxxxxx）"
                value={apiKeyInput}
                onChange={(event) => setApiKeyInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') handleSubmit()
                }}
                className="h-10 text-sm"
              />
            </div>

            <div className="flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-center sm:justify-center">
              <Button
                className="w-full bg-violet-600 text-white hover:bg-violet-700 sm:w-auto sm:min-w-[120px]"
                onClick={handleSubmit}
                disabled={!canSubmit}
              >
                <Search className="mr-2 h-4 w-4" />
                查询统计
              </Button>
              <Button
                variant="outline"
                className="w-full border-neutral-200 text-neutral-700 hover:bg-neutral-50 hover:text-neutral-900 sm:w-auto sm:min-w-[120px]"
                onClick={() => {
                  if (!hasServiceKey) return
                  void fetch({ userServiceKey: filters.userServiceKey })
                }}
                disabled={!hasServiceKey || pageBusy}
              >
                <RefreshCw className="mr-2 h-4 w-4" />
                刷新
              </Button>
              <Button
                variant="ghost"
                className="w-full text-neutral-700 hover:bg-neutral-100 hover:text-neutral-900 sm:w-auto sm:min-w-[120px]"
                onClick={() => {
                  setApiKeyInput('')
                  setTrendTimeframe('7d')
                  setModelScope('today')
                  clear()
                }}
                disabled={pageBusy}
              >
                <RotateCcw className="mr-2 h-4 w-4" />
                重置
              </Button>
            </div>

            {error ? (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
          </CardContent>
        </Card>

        <section className="space-y-6">
          <StatsOverview metrics={summary} loading={loading} hasFetched={hasFetched} />

          <div className="grid gap-6 lg:grid-cols-2">
            <StatsTrendChart
              data={trend}
              loading={loading || trendLoading}
              hasFetched={hasFetched}
              timeframe={trendTimeframe}
              timeframeOptions={TREND_OPTIONS}
              onTimeframeChange={(value) => {
                setTrendTimeframe(value)
                void fetchTrendOnly(value)
              }}
            />

            <StatsModelShare
              data={modelShare}
              loading={loading || modelShareLoading}
              hasFetched={hasFetched}
              scope={modelScope}
              onScopeChange={(scope) => {
                setModelScope(scope)
                void fetchModelShareOnly(scope === 'today')
              }}
            />
          </div>

          <StatsLogsTable
            logs={logs}
            loading={loading}
            hasFetched={hasFetched}
            onPageChange={(page) => {
              if (!hasServiceKey) return
              setFilters({ page })
              void fetch({ userServiceKey: filters.userServiceKey, page })
            }}
          />
        </section>
      </div>
    </div>
  )
}
