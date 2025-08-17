/**
 * Dashboard.tsx
 * 仪表板首页：提供关键指标卡片与简易概览，保证首页不为空白。
 */

import React, { useState, useMemo } from 'react'
import { Activity, Timer, Coins, CheckCircle2, Calendar, ChevronDown, TrendingUp, BarChart } from 'lucide-react'

/** 指标项接口 */
interface StatItem {
  key: string
  label: string
  value: string
  delta: string
  icon: React.ReactNode
  color: string
}

/** 时间范围类型 */
type TimeRange = 'today' | '7days' | '30days' | 'custom'

/** 模型使用数据接口 */
interface ModelUsage {
  name: string
  count: number
  percentage: number
  cost: number
  color: string
}

/** 自定义日期范围接口 */
interface CustomDateRange {
  startDate: string
  endDate: string
}

/** 趋势图数据点接口 */
interface TrendDataPoint {
  date: string
  requests: number
  tokens: number
}

/** 趋势图显示模式 */
type TrendViewMode = 'requests' | 'tokens'

/** 指标卡片组件 */
const StatCard: React.FC<{ item: StatItem }> = ({ item }) => {
  return (
    <div className="group relative overflow-hidden rounded-2xl border border-neutral-200 bg-white p-4 shadow-sm transition hover:shadow-md">
      {/* 顶部色条 */}
      <div className="absolute inset-x-0 top-0 h-1" style={{ backgroundColor: item.color }} />
      <div className="flex items-center gap-3">
        <div
          className="flex h-10 w-10 items-center justify-center rounded-xl text-white"
          style={{ backgroundColor: item.color }}
          aria-hidden
        >
          {item.icon}
        </div>
        <div className="min-w-0">
          <div className="text-sm text-neutral-500">{item.label}</div>
          <div className="flex items-baseline gap-2">
            <div className="truncate text-xl font-semibold text-neutral-900">{item.value}</div>
            <div className="text-xs text-emerald-600">{item.delta}</div>
          </div>
        </div>
      </div>
    </div>
  )
}

/** 趋势图组件 */
const TrendChart: React.FC<{ 
  data: TrendDataPoint[] 
  viewMode: TrendViewMode
  onViewModeChange: (mode: TrendViewMode) => void
  title: string
  color: string
}> = ({ data, viewMode, onViewModeChange, title, color }) => {
  const maxValue = Math.max(...data.map(d => viewMode === 'requests' ? d.requests : d.tokens))
  
  // 生成SVG路径
  const generatePath = (points: number[]) => {
    if (points.length === 0) return ''
    
    const width = 600
    const height = 200
    const padding = 40
    
    const xStep = (width - padding * 2) / (points.length - 1)
    const yScale = (height - padding * 2) / maxValue
    
    let path = `M ${padding} ${height - padding - points[0] * yScale}`
    
    for (let i = 1; i < points.length; i++) {
      const x = padding + i * xStep
      const y = height - padding - points[i] * yScale
      path += ` L ${x} ${y}`
    }
    
    return path
  }
  
  const currentData = data.map(d => viewMode === 'requests' ? d.requests : d.tokens)
  const pathData = generatePath(currentData)
  
  return (
    <div className="space-y-4">
      {/* 标题和切换按钮 */}
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-neutral-900">{title}</h4>
        <div className="flex rounded-lg border border-neutral-200 bg-white">
          <button
            onClick={() => onViewModeChange('requests')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-l-lg transition-colors ${
              viewMode === 'requests' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <BarChart size={12} />
            请求次数
          </button>
          <button
            onClick={() => onViewModeChange('tokens')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-r-lg transition-colors ${
              viewMode === 'tokens' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <Coins size={12} />
            Token数量
          </button>
        </div>
      </div>
      
      {/* 趋势图 */}
      <div className="relative">
        <svg width="600" height="200" className="w-full">
          {/* 网格线 */}
          <defs>
            <pattern id="grid" width="60" height="40" patternUnits="userSpaceOnUse">
              <path d="M 60 0 L 0 0 0 40" fill="none" stroke="#f3f4f6" strokeWidth="1"/>
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#grid)" />
          
          {/* 趋势线 */}
          <path
            d={pathData}
            fill="none"
            stroke={color}
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="drop-shadow-sm"
          />
          
          {/* 数据点 */}
          {currentData.map((value, index) => {
            const width = 600
            const height = 200
            const padding = 40
            const xStep = (width - padding * 2) / (currentData.length - 1)
            const yScale = (height - padding * 2) / maxValue
            const x = padding + index * xStep
            const y = height - padding - value * yScale
            
            return (
              <circle
                key={index}
                cx={x}
                cy={y}
                r="4"
                fill={color}
                className="hover:r-6 transition-all cursor-pointer"
                title={`${data[index].date}: ${value.toLocaleString()}`}
              />
            )
          })}
        </svg>
        
        {/* X轴标签 */}
        <div className="flex justify-between mt-2 px-10 text-xs text-neutral-500">
          {data.map((item, index) => (
            <span key={index} className="text-center">
              {new Date(item.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })}
            </span>
          ))}
        </div>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {currentData[currentData.length - 1]?.toLocaleString() || 0}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.round(currentData.reduce((sum, val) => sum + val, 0) / currentData.length).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.max(...currentData).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 简化的Token趋势图组件 */
const SimpleTokenChart: React.FC<{
  data: { date: string; value: number }[]
}> = ({ data }) => {
  const values = data.map(d => d.value)
  const maxValue = Math.max(...values)
  
  // 生成SVG路径
  const generatePath = (points: number[]) => {
    if (points.length === 0) return ''
    
    const width = 600
    const height = 200
    const padding = 40
    
    const xStep = (width - padding * 2) / (points.length - 1)
    const yScale = (height - padding * 2) / maxValue
    
    let path = `M ${padding} ${height - padding - points[0] * yScale}`
    
    for (let i = 1; i < points.length; i++) {
      const x = padding + i * xStep
      const y = height - padding - points[i] * yScale
      path += ` L ${x} ${y}`
    }
    
    return path
  }
  
  const pathData = generatePath(values)
  
  // 格式化Token数值
  const formatTokenValue = (value: number) => {
    if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`
    if (value >= 1000) return `${(value / 1000).toFixed(1)}K`
    return value.toString()
  }

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="relative">
        <svg width="600" height="200" className="w-full">
          {/* 网格线 */}
          <defs>
            <pattern id="tokenGrid" width="60" height="40" patternUnits="userSpaceOnUse">
              <path d="M 60 0 L 0 0 0 40" fill="none" stroke="#f3f4f6" strokeWidth="1"/>
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#tokenGrid)" />
          
          {/* 渐变填充 */}
          <defs>
            <linearGradient id="tokenGradient" x1="0%" y1="0%" x2="0%" y2="100%">
              <stop offset="0%" stopColor="#0ea5e9" stopOpacity="0.3"/>
              <stop offset="100%" stopColor="#0ea5e9" stopOpacity="0"/>
            </linearGradient>
          </defs>
          
          {/* 填充区域 */}
          <path
            d={`${pathData} L ${40 + (values.length - 1) * ((600 - 80) / (values.length - 1))} 160 L 40 160 Z`}
            fill="url(#tokenGradient)"
          />
          
          {/* 趋势线 */}
          <path
            d={pathData}
            fill="none"
            stroke="#0ea5e9"
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="drop-shadow-sm"
          />
          
          {/* 数据点 */}
          {values.map((value, index) => {
            const width = 600
            const height = 200
            const padding = 40
            const xStep = (width - padding * 2) / (values.length - 1)
            const yScale = (height - padding * 2) / maxValue
            const x = padding + index * xStep
            const y = height - padding - value * yScale
            
            return (
              <circle
                key={index}
                cx={x}
                cy={y}
                r="4"
                fill="#0ea5e9"
                stroke="white"
                strokeWidth="2"
                className="hover:r-6 transition-all cursor-pointer"
                title={`${data[index].date}: ${value.toLocaleString()}`}
              />
            )
          })}
        </svg>
        
        {/* X轴标签 */}
        <div className="flex justify-between mt-2 px-10 text-xs text-neutral-500">
          {data.map((item, index) => (
            <span key={index} className="text-center">
              {new Date(item.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })}
            </span>
          ))}
        </div>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(values[values.length - 1] || 0)}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(Math.round(values.reduce((sum, val) => sum + val, 0) / values.length))}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {formatTokenValue(Math.max(...values))}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 无控制按钮的趋势图组件 */
const TrendChartWithoutControls: React.FC<{
  data: TrendDataPoint[]
  viewMode: TrendViewMode
  color: string
}> = ({ data, viewMode, color }) => {
  const currentData = data.map(d => viewMode === 'requests' ? d.requests : d.tokens)
  const maxValue = Math.max(...currentData)
  
  // 生成SVG路径
  const generatePath = (points: number[]) => {
    if (points.length === 0) return ''
    
    const width = 600
    const height = 200
    const padding = 40
    
    const xStep = (width - padding * 2) / (points.length - 1)
    const yScale = (height - padding * 2) / maxValue
    
    let path = `M ${padding} ${height - padding - points[0] * yScale}`
    
    for (let i = 1; i < points.length; i++) {
      const x = padding + i * xStep
      const y = height - padding - points[i] * yScale
      path += ` L ${x} ${y}`
    }
    
    return path
  }
  
  const pathData = generatePath(currentData)

  return (
    <div className="space-y-4">
      {/* 图表 */}
      <div className="relative">
        <svg width="600" height="200" className="w-full">
          {/* 网格线 */}
          <defs>
            <pattern id="userApiGrid" width="60" height="40" patternUnits="userSpaceOnUse">
              <path d="M 60 0 L 0 0 0 40" fill="none" stroke="#f3f4f6" strokeWidth="1"/>
            </pattern>
          </defs>
          <rect width="100%" height="100%" fill="url(#userApiGrid)" />
          
          {/* 趋势线 */}
          <path
            d={pathData}
            fill="none"
            stroke={color}
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="drop-shadow-sm"
          />
          
          {/* 数据点 */}
          {currentData.map((value, index) => {
            const width = 600
            const height = 200
            const padding = 40
            const xStep = (width - padding * 2) / (currentData.length - 1)
            const yScale = (height - padding * 2) / maxValue
            const x = padding + index * xStep
            const y = height - padding - value * yScale
            
            return (
              <circle
                key={index}
                cx={x}
                cy={y}
                r="4"
                fill={color}
                className="hover:r-6 transition-all cursor-pointer"
                title={`${data[index].date}: ${value.toLocaleString()}`}
              />
            )
          })}
        </svg>
        
        {/* X轴标签 */}
        <div className="flex justify-between mt-2 px-10 text-xs text-neutral-500">
          {data.map((item, index) => (
            <span key={index} className="text-center">
              {new Date(item.date).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })}
            </span>
          ))}
        </div>
      </div>
      
      {/* 统计信息 */}
      <div className="grid grid-cols-3 gap-4 pt-3 border-t border-neutral-100">
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {currentData[currentData.length - 1]?.toLocaleString() || 0}
          </div>
          <div className="text-xs text-neutral-500">最新值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.round(currentData.reduce((sum, val) => sum + val, 0) / currentData.length).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">平均值</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-bold text-neutral-900">
            {Math.max(...currentData).toLocaleString()}
          </div>
          <div className="text-xs text-neutral-500">峰值</div>
        </div>
      </div>
    </div>
  )
}

/** 紧凑型时间选择器组件 */
const CompactTimeRangeSelector: React.FC<{
  selectedRange: TimeRange
  customRange: CustomDateRange
  onRangeChange: (range: TimeRange) => void
  onCustomRangeChange: (range: CustomDateRange) => void
}> = ({ selectedRange, customRange, onRangeChange, onCustomRangeChange }) => {
  const [showDropdown, setShowDropdown] = useState(false)
  const [showCustomPicker, setShowCustomPicker] = useState(false)

  const timeRangeOptions = [
    { value: 'today' as TimeRange, label: '今天' },
    { value: '7days' as TimeRange, label: '最近7天' },
    { value: '30days' as TimeRange, label: '最近30天' },
    { value: 'custom' as TimeRange, label: '自定义时间' },
  ]

  const getCurrentLabel = () => {
    const option = timeRangeOptions.find(opt => opt.value === selectedRange)
    if (selectedRange === 'custom') {
      return `${customRange.startDate} 至 ${customRange.endDate}`
    }
    return option?.label || '选择时间范围'
  }

  return (
    <div className="relative">
      <button
        onClick={() => setShowDropdown(!showDropdown)}
        className="flex items-center gap-1 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs hover:bg-neutral-50"
      >
        <Calendar size={12} className="text-neutral-500" />
        <span>{getCurrentLabel()}</span>
        <ChevronDown size={10} className="text-neutral-400" />
      </button>

      {showDropdown && (
        <div className="absolute right-0 z-10 mt-1 w-48 rounded-lg border border-neutral-200 bg-white shadow-lg">
          <div className="p-1">
            {timeRangeOptions.map((option) => (
              <button
                key={option.value}
                onClick={() => {
                  onRangeChange(option.value)
                  if (option.value === 'custom') {
                    setShowCustomPicker(true)
                  } else {
                    setShowCustomPicker(false)
                  }
                  setShowDropdown(false)
                }}
                className={`w-full rounded px-3 py-2 text-left text-sm hover:bg-neutral-50 ${
                  selectedRange === option.value ? 'bg-violet-50 text-violet-700' : 'text-neutral-700'
                }`}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}

      {showCustomPicker && selectedRange === 'custom' && (
        <div className="absolute right-0 z-20 mt-1 w-80 rounded-lg border border-neutral-200 bg-white p-4 shadow-lg">
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">开始日期</label>
              <input
                type="date"
                value={customRange.startDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, startDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-neutral-700 mb-1">结束日期</label>
              <input
                type="date"
                value={customRange.endDate}
                onChange={(e) => onCustomRangeChange({ ...customRange, endDate: e.target.value })}
                className="w-full rounded border border-neutral-200 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500/40"
              />
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-neutral-100 px-3 py-2 text-sm text-neutral-600 hover:bg-neutral-200"
              >
                取消
              </button>
              <button
                onClick={() => setShowCustomPicker(false)}
                className="flex-1 rounded bg-violet-600 px-3 py-2 text-sm text-white hover:bg-violet-700"
              >
                确认
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

/** 饼图组件 */
const PieChart: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const total = data.reduce((sum, item) => sum + item.count, 0)
  let currentAngle = 0

  const createPath = (startAngle: number, endAngle: number) => {
    const centerX = 160
    const centerY = 160
    const radius = 120

    const x1 = centerX + radius * Math.cos(startAngle)
    const y1 = centerY + radius * Math.sin(startAngle)
    const x2 = centerX + radius * Math.cos(endAngle)
    const y2 = centerY + radius * Math.sin(endAngle)

    const largeArcFlag = endAngle - startAngle <= Math.PI ? "0" : "1"

    return `M ${centerX} ${centerY} L ${x1} ${y1} A ${radius} ${radius} 0 ${largeArcFlag} 1 ${x2} ${y2} Z`
  }

  // 智能处理模型数据显示
  const processedData = useMemo(() => {
    // 按使用量排序
    const sortedData = [...data].sort((a, b) => b.count - a.count)
    
    // 如果模型数量少于等于4个，直接显示全部
    if (sortedData.length <= 4) return sortedData
    
    // 如果有5-6个模型，全部显示
    if (sortedData.length <= 6) return sortedData
    
    // 如果超过6个模型，显示前5个，其余合并为"其他"
    const topModels = sortedData.slice(0, 5)
    const otherModels = sortedData.slice(5)
    
    // 计算"其他"的占比，如果太小（<3%）则合并到前一个模型
    const otherTotal = otherModels.reduce((sum, item) => sum + item.count, 0)
    const otherPercentage = (otherTotal / total) * 100
    
    if (otherPercentage < 3) {
      // 如果"其他"占比太小，显示前6个，不显示"其他"
      return sortedData.slice(0, 6)
    }
    
    const otherCost = otherModels.reduce((sum, item) => sum + item.cost, 0)
    
    return [
      ...topModels,
      {
        name: `其他 (${otherModels.length}个)`,
        count: otherTotal,
        cost: otherCost,
        percentage: otherPercentage,
        color: '#6b7280'
      }
    ]
  }, [data, total])

  // 检查是否有数据
  if (!data.length || total === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-80 text-neutral-400">
        <div className="text-center">
          <div className="text-4xl mb-2">📊</div>
          <div className="text-sm">暂无模型使用数据</div>
        </div>
      </div>
    )
  }

  // 动态调整图例布局：少量模型用单列，多模型用双列
  const legendCols = processedData.length <= 3 ? 1 : 2

  return (
    <div className="flex flex-col items-center gap-6">
      <div className="relative">
        <svg width="320" height="320" className="transform -rotate-90">
          {processedData.map((item, index) => {
            const percentage = (item.count / total) * 100
            const startAngle = currentAngle
            const endAngle = currentAngle + (percentage / 100) * 2 * Math.PI
            currentAngle = endAngle

            // 过滤掉极小的片段（<0.5%），避免视觉噪音
            if (percentage < 0.5) return null

            return (
              <g key={index}>
                <path
                  d={createPath(startAngle, endAngle)}
                  fill={item.color}
                  className="transition-all duration-200 hover:opacity-80 cursor-pointer"
                  style={{
                    filter: 'drop-shadow(0 2px 4px rgba(0,0,0,0.1))'
                  }}
                />
                {/* 为小片段添加标签线（当片段太小时） */}
                {percentage > 0.5 && percentage < 5 && (
                  <g className="pointer-events-none">
                    <line
                      x1={160 + 90 * Math.cos((startAngle + endAngle) / 2)}
                      y1={160 + 90 * Math.sin((startAngle + endAngle) / 2)}
                      x2={160 + 130 * Math.cos((startAngle + endAngle) / 2)}
                      y2={160 + 130 * Math.sin((startAngle + endAngle) / 2)}
                      stroke={item.color}
                      strokeWidth="1"
                      className="opacity-60"
                    />
                  </g>
                )}
              </g>
            )
          })}
        </svg>
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="text-center">
            <div className="text-3xl font-bold text-neutral-900">{total.toLocaleString()}</div>
            <div className="text-sm text-neutral-500 mt-1">总请求数</div>
          </div>
        </div>
      </div>
      
      <div className={`w-full grid gap-2 ${legendCols === 1 ? 'grid-cols-1' : 'grid-cols-2'}`}>
        {processedData.map((item, index) => (
          <div key={index} className="flex items-center gap-2">
            <div
              className="h-3 w-3 rounded-full flex-shrink-0"
              style={{ backgroundColor: item.color }}
            />
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between">
                <span 
                  className="text-sm font-medium text-neutral-700 truncate" 
                  title={item.name}
                >
                  {item.name}
                </span>
                <span className="text-sm text-neutral-500 ml-2 flex-shrink-0">
                  {item.percentage.toFixed(1)}%
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
      
      {/* 如果显示了"其他"分类，添加提示 */}
      {processedData.some(item => item.name.includes('其他')) && (
        <div className="text-xs text-neutral-400 text-center">
          * "其他"包含使用量较少的模型，详情请查看右侧统计列表
        </div>
      )}
    </div>
  )
}

/** 模型统计列表组件 */
const ModelStatsList: React.FC<{ data: ModelUsage[] }> = ({ data }) => {
  const [showAll, setShowAll] = useState(false)
  const sortedData = [...data].sort((a, b) => b.count - a.count)
  
  // 默认显示前5个，可展开查看全部
  const displayData = showAll ? sortedData : sortedData.slice(0, 5)
  const hasMore = sortedData.length > 5

  return (
    <div className="space-y-3">
      <div className="max-h-96 overflow-y-auto space-y-3">
        {displayData.map((item, index) => (
          <div key={index} className="flex items-center justify-between rounded-lg border border-neutral-100 p-3 hover:bg-neutral-50 transition-colors">
            <div className="flex items-center gap-3">
              <div
                className="h-4 w-4 rounded-full flex-shrink-0"
                style={{ backgroundColor: item.color }}
              />
              <div className="min-w-0">
                <div className="font-medium text-neutral-900 truncate">{item.name}</div>
                <div className="text-sm text-neutral-500">{item.count.toLocaleString()} 次请求</div>
              </div>
            </div>
            <div className="text-right flex-shrink-0">
              <div className="font-medium text-neutral-900">${item.cost.toFixed(2)}</div>
              <div className="text-sm text-neutral-500">{item.percentage.toFixed(1)}%</div>
            </div>
          </div>
        ))}
      </div>
      
      {hasMore && (
        <div className="pt-3 mt-2 border-t border-neutral-100">
          <button
            onClick={() => setShowAll(!showAll)}
            className="w-full flex items-center justify-center gap-2 py-3 text-sm text-neutral-600 hover:text-neutral-800 hover:bg-neutral-50 rounded-lg transition-colors"
          >
            <span>{showAll ? '收起' : `查看全部 ${sortedData.length} 个模型`}</span>
            <svg 
              className={`h-4 w-4 transition-transform ${showAll ? 'rotate-180' : ''}`} 
              fill="none" 
              viewBox="0 0 24 24" 
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        </div>
      )}
    </div>
  )
}

/**
 * DashboardPage
 * - 4 个指标卡
 * - 欢迎区 + 图表
 */
/** 带独立时间筛选的饼图组件 */
const PieChartWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围生成模拟数据
  const generateModelData = useMemo(() => {
    const baseData = [
      { name: 'GPT-4', color: '#7c3aed', baseCount: 5420, baseCost: 125.50 },
      { name: 'GPT-3.5 Turbo', color: '#0ea5e9', baseCount: 3210, baseCost: 45.20 },
      { name: 'Claude-3', color: '#10b981', baseCount: 2150, baseCost: 89.30 },
      { name: 'Gemini Pro', color: '#f59e0b', baseCount: 1890, baseCost: 67.80 },
      { name: 'PaLM-2', color: '#ef4444', baseCount: 980, baseCost: 32.40 },
      { name: 'GPT-4 Turbo', color: '#8b5cf6', baseCount: 780, baseCost: 28.90 },
      { name: 'Claude-2', color: '#06b6d4', baseCount: 650, baseCost: 23.10 },
      { name: 'Llama-2', color: '#84cc16', baseCount: 420, baseCost: 15.60 },
      { name: 'Code Llama', color: '#f97316', baseCount: 320, baseCost: 12.30 },
      { name: 'Mistral-7B', color: '#ec4899', baseCount: 180, baseCost: 8.50 },
      { name: 'Vicuna-13B', color: '#14b8a6', baseCount: 150, baseCost: 6.80 },
      { name: 'Alpaca-7B', color: '#a855f7', baseCount: 120, baseCost: 4.20 },
      { name: 'ChatGLM-6B', color: '#f43f5e', baseCount: 95, baseCost: 3.50 },
      { name: 'Falcon-7B', color: '#22c55e', baseCount: 80, baseCost: 2.80 },
      { name: 'StableLM-7B', color: '#3b82f6', baseCount: 60, baseCost: 2.10 },
    ]

    // 根据时间范围调整数据
    const multiplier = selectedTimeRange === 'today' ? 0.3 : 
                     selectedTimeRange === '7days' ? 1 : 
                     selectedTimeRange === '30days' ? 4.2 : 1

    const adjustedData = baseData.map(item => ({
      ...item,
      count: Math.round(item.baseCount * multiplier),
      cost: item.baseCost * multiplier
    }))

    const total = adjustedData.reduce((sum, item) => sum + item.count, 0)

    return adjustedData.map(item => ({
      ...item,
      percentage: (item.count / total) * 100
    }))
  }, [selectedTimeRange])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用占比</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      <PieChart data={generateModelData} />
    </div>
  )
}

/** 带独立时间筛选的统计列表组件 */
const ModelStatsListWithTimeFilter: React.FC = () => {
  const [selectedTimeRange, setSelectedTimeRange] = useState<TimeRange>('7days')
  const [customDateRange, setCustomDateRange] = useState<CustomDateRange>({
    startDate: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0],
    endDate: new Date().toISOString().split('T')[0]
  })

  // 根据时间范围生成模拟数据 (复用相同逻辑)
  const generateModelData = useMemo(() => {
    const baseData = [
      { name: 'GPT-4', color: '#7c3aed', baseCount: 5420, baseCost: 125.50 },
      { name: 'GPT-3.5 Turbo', color: '#0ea5e9', baseCount: 3210, baseCost: 45.20 },
      { name: 'Claude-3', color: '#10b981', baseCount: 2150, baseCost: 89.30 },
      { name: 'Gemini Pro', color: '#f59e0b', baseCount: 1890, baseCost: 67.80 },
      { name: 'PaLM-2', color: '#ef4444', baseCount: 980, baseCost: 32.40 },
      { name: 'GPT-4 Turbo', color: '#8b5cf6', baseCount: 780, baseCost: 28.90 },
      { name: 'Claude-2', color: '#06b6d4', baseCount: 650, baseCost: 23.10 },
      { name: 'Llama-2', color: '#84cc16', baseCount: 420, baseCost: 15.60 },
      { name: 'Code Llama', color: '#f97316', baseCount: 320, baseCost: 12.30 },
      { name: 'Mistral-7B', color: '#ec4899', baseCount: 180, baseCost: 8.50 },
      { name: 'Vicuna-13B', color: '#14b8a6', baseCount: 150, baseCost: 6.80 },
      { name: 'Alpaca-7B', color: '#a855f7', baseCount: 120, baseCost: 4.20 },
      { name: 'ChatGLM-6B', color: '#f43f5e', baseCount: 95, baseCost: 3.50 },
      { name: 'Falcon-7B', color: '#22c55e', baseCount: 80, baseCost: 2.80 },
      { name: 'StableLM-7B', color: '#3b82f6', baseCount: 60, baseCost: 2.10 },
    ]

    // 根据时间范围调整数据
    const multiplier = selectedTimeRange === 'today' ? 0.3 : 
                     selectedTimeRange === '7days' ? 1 : 
                     selectedTimeRange === '30days' ? 4.2 : 1

    const adjustedData = baseData.map(item => ({
      ...item,
      count: Math.round(item.baseCount * multiplier),
      cost: item.baseCost * multiplier
    }))

    const total = adjustedData.reduce((sum, item) => sum + item.count, 0)

    return adjustedData.map(item => ({
      ...item,
      percentage: (item.count / total) * 100
    }))
  }, [selectedTimeRange])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-medium text-neutral-900">模型使用统计</h3>
        <CompactTimeRangeSelector
          selectedRange={selectedTimeRange}
          customRange={customDateRange}
          onRangeChange={setSelectedTimeRange}
          onCustomRangeChange={setCustomDateRange}
        />
      </div>
      <ModelStatsList data={generateModelData} />
    </div>
  )
}

/** Token使用趋势图组件 */
const TokenTrendChart: React.FC = () => {
  // 生成最近30天的Token消耗数据
  const generateTokenData = useMemo(() => {
    const days = 30
    const data: { date: string; value: number }[] = []
    const baseTokens = 125000

    for (let i = days - 1; i >= 0; i--) {
      const date = new Date()
      date.setDate(date.getDate() - i)
      
      // 添加一些随机波动和趋势
      const trendFactor = 1 + (days - i) * 0.015 // 轻微上升趋势
      const weekdayFactor = [0, 6].includes(date.getDay()) ? 0.7 : 1.0 // 周末较少
      const randomFactor = 0.8 + Math.random() * 0.4 // 随机波动
      
      data.push({
        date: date.toISOString(),
        value: Math.round(baseTokens * trendFactor * weekdayFactor * randomFactor)
      })
    }

    return data
  }, [])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4">
        <h3 className="text-sm font-medium text-neutral-900">Token使用趋势</h3>
        <p className="text-xs text-neutral-500 mt-1">最近30天Token消耗数量</p>
      </div>
      <SimpleTokenChart data={generateTokenData} />
    </div>
  )
}

/** 用户API Keys使用趋势图组件 */
const UserApiKeysTrendChart: React.FC = () => {
  const [viewMode, setViewMode] = useState<TrendViewMode>('requests')

  // 生成最近30天的趋势数据
  const generateTrendData = useMemo(() => {
    const days = 30
    const data: TrendDataPoint[] = []
    const baseRequests = 12400
    const baseTokens = 186000

    for (let i = days - 1; i >= 0; i--) {
      const date = new Date()
      date.setDate(date.getDate() - i)
      
      // 不同的趋势模式 - 用户API Keys可能有不同的使用模式
      const weekdayFactor = [0, 6].includes(date.getDay()) ? 0.6 : 1.1 // 周末较少
      const trendFactor = 1 + (days - i) * 0.015 // 温和上升趋势
      const randomFactor = 0.7 + Math.random() * 0.6 // 更大的随机波动
      
      data.push({
        date: date.toISOString(),
        requests: Math.round(baseRequests * trendFactor * weekdayFactor * randomFactor),
        tokens: Math.round(baseTokens * trendFactor * weekdayFactor * randomFactor)
      })
    }

    return data
  }, [])

  return (
    <div className="rounded-2xl border border-neutral-200 bg-white p-6">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-neutral-900">用户API Keys使用趋势</h3>
          <p className="text-xs text-neutral-500 mt-1">最近30天数据</p>
        </div>
        
        {/* 切换按钮移动到右上方 */}
        <div className="flex rounded-lg border border-neutral-200 bg-white">
          <button
            onClick={() => setViewMode('requests')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-l-lg transition-colors ${
              viewMode === 'requests' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <BarChart size={12} />
            请求次数
          </button>
          <button
            onClick={() => setViewMode('tokens')}
            className={`flex items-center gap-1 px-3 py-1 text-xs rounded-r-lg transition-colors ${
              viewMode === 'tokens' 
                ? 'bg-violet-100 text-violet-700' 
                : 'text-neutral-600 hover:text-neutral-800'
            }`}
          >
            <Coins size={12} />
            Token数量
          </button>
        </div>
      </div>
      <TrendChartWithoutControls
        data={generateTrendData}
        viewMode={viewMode}
        color="#7c3aed"
      />
    </div>
  )
}

const DashboardPage: React.FC = () => {
  const stats: StatItem[] = [
    {
      key: 'requests',
      label: '今日请求数',
      value: '12,432',
      delta: '+6.4%',
      icon: <Activity size={18} />,
      color: '#7c3aed',
    },
    {
      key: 'tokens',
      label: '今日 Token 消耗',
      value: '184,230',
      delta: '+4.1%',
      icon: <Coins size={18} />,
      color: '#0ea5e9',
    },
    {
      key: 'latency',
      label: '平均响应时间',
      value: '482 ms',
      delta: '-3.2%',
      icon: <Timer size={18} />,
      color: '#f59e0b',
    },
    {
      key: 'success',
      label: '成功率',
      value: '98.7%',
      delta: '+0.5%',
      icon: <CheckCircle2 size={18} />,
      color: '#10b981',
    },
  ]

  return (
    <div className="w-full">
      {/* 欢迎区 */}
      <section className="mb-6 rounded-2xl border border-neutral-200 bg-gradient-to-r from-violet-50 to-indigo-50 p-5">
        <h2 className="text-lg font-semibold text-neutral-900">欢迎回来 👋</h2>
        <p className="mt-1 text-sm text-neutral-600">
          这里是系统运行概览与关键指标。更多分析请前往各功能页面。
        </p>
      </section>

      {/* 指标卡片 */}
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((s) => (
          <StatCard key={s.key} item={s} />
        ))}
      </section>

      {/* 模型使用分析 - 2列布局 */}
      <section className="mt-6 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <PieChartWithTimeFilter />
        <ModelStatsListWithTimeFilter />
      </section>

      {/* 趋势分析 - 每个图表独占一行 */}
      <section className="mt-6 space-y-4">
        <TokenTrendChart />
        <UserApiKeysTrendChart />
      </section>
    </div>
  )
}

export default DashboardPage
