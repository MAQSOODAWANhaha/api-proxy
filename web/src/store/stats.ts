import { create } from 'zustand'
import { formatISO, startOfDay, subDays } from 'date-fns'
import { statsApi } from '@/lib/stats'
import type { StatsResponse, SummaryMetric, TrendPoint, ModelShareItem, LogsPage } from '@/types/stats'
import { useTimezoneStore } from './timezone'

export type RangePreset = 'today' | '7d' | '30d' | 'custom'
export type Timeframe = '90d' | '30d' | '7d'

export interface StatsFilters {
  userServiceKey: string
  rangePreset: RangePreset
  timeframe: Timeframe
  from?: string
  to?: string
  page: number
  pageSize: number
}

export interface StatsState {
  filters: StatsFilters
  loading: boolean
  error: string | null
  hasFetched: boolean
  summary: SummaryMetric[]
  trend: TrendPoint[]
  modelShare: {
    today: ModelShareItem[]
    total: ModelShareItem[]
  }
  logs: LogsPage | null
  setFilters: (updater: Partial<StatsFilters> | ((draft: StatsFilters) => void)) => void
  resetPagination: () => void
  setTimeframe: (timeframe: Timeframe) => void
  fetch: (overrides?: Partial<StatsFilters>) => Promise<void>
  clear: () => void
}

const buildRange = (preset: RangePreset): { from: string; to: string } => {
  const now = new Date()
  switch (preset) {
    case 'today':
      return { from: formatISO(startOfDay(now)), to: formatISO(now) }
    case '7d':
      return { from: formatISO(subDays(now, 7)), to: formatISO(now) }
    case '30d':
      return { from: formatISO(subDays(now, 30)), to: formatISO(now) }
    default:
      return { from: formatISO(subDays(now, 1)), to: formatISO(now) }
  }
}

const initialRange = buildRange('today')

export const useStatsStore = create<StatsState>((set, get) => ({
  filters: {
    userServiceKey: '',
    rangePreset: 'today',
    timeframe: '90d',
    from: initialRange.from,
    to: initialRange.to,
    page: 1,
    pageSize: 20,
  },
  loading: false,
  error: null,
  hasFetched: false,
  summary: [],
  trend: [],
  modelShare: {
    today: [],
    total: [],
  },
  logs: null,

  setFilters: (updater) =>
    set((state) => {
      const draft = { ...state.filters }
      if (typeof updater === 'function') {
        updater(draft)
      } else {
        Object.assign(draft, updater)
      }
      return { filters: draft }
    }),

  resetPagination: () =>
    set((state) => ({
      filters: { ...state.filters, page: 1 },
    })),

  setTimeframe: (timeframe) =>
    set((state) => ({
      filters: { ...state.filters, timeframe },
    })),

  clear: () =>
    set((state) => ({
      summary: [],
      trend: [],
      modelShare: { today: [], total: [] },
      logs: null,
      hasFetched: false,
      error: null,
      filters: { ...state.filters, page: 1 },
    })),

  fetch: async (overrides) => {
    const { filters } = get()
    const nextFilters = { ...filters, ...overrides }
    const timezone = useTimezoneStore.getState().timezone

    if (!nextFilters.userServiceKey.trim()) {
      set({ error: '请先输入用户 API Key', hasFetched: false })
      return
    }

    const range =
      nextFilters.rangePreset === 'custom' && nextFilters.from && nextFilters.to
        ? { from: nextFilters.from, to: nextFilters.to }
        : buildRange(nextFilters.rangePreset)

    set({ loading: true, error: null })

    try {
      const response = await statsApi.fetchStats({
        user_service_key: nextFilters.userServiceKey,
        from: range.from,
        to: range.to,
        page: nextFilters.page,
        page_size: nextFilters.pageSize,
      })

      if (!response.success || !response.data) {
        throw new Error(response.error?.message || '查询失败')
      }

      const data: StatsResponse = response.data

      set({
        filters: {
          ...nextFilters,
          from: range.from,
          to: range.to,
        },
        summary: data.summary,
        trend: data.trend,
        modelShare: {
          today: data.model_share.today ?? [],
          total: data.model_share.total ?? [],
        },
        logs: data.logs,
        loading: false,
        error: null,
        hasFetched: true,
      })
    } catch (error) {
      const message = error instanceof Error ? error.message : '网络异常'
      set({ loading: false, error: message, hasFetched: false })
    } finally {
      if (!timezone) {
        useTimezoneStore.getState().detectTimezone()
      }
    }
  },
}))
