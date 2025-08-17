/**
 * Pagination.tsx
 * 通用分页组件：支持首页/上一页/页码/下一页/末页、页大小切换。
 */

import React, { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { ChevronsLeft, ChevronLeft, ChevronRight, ChevronsRight } from 'lucide-react'
import ModernSelect from './ModernSelect'

/** 分页组件 Props */
export interface PaginationProps {
  /** 总条数 */
  total: number
  /** 当前页（从 1 开始） */
  page: number
  /** 每页条数 */
  pageSize: number
  /** 页码变更回调 */
  onPageChange: (page: number) => void
  /** 是否显示页大小切换 */
  showSizeChanger?: boolean
  /** 可选页大小集合 */
  pageSizeOptions?: number[]
  /** 页大小变更回调 */
  onPageSizeChange?: (size: number) => void
  /** 额外样式 */
  className?: string
}

/** 生成显示的页码序列（含省略号） */
function getPageItems(totalPages: number, current: number): Array<number | '...'> {
  if (totalPages <= 7) {
    return Array.from({ length: totalPages }, (_, i) => i + 1)
  }
  const items: Array<number | '...'> = []
  items.push(1)
  const left = Math.max(2, current - 1)
  const right = Math.min(totalPages - 1, current + 1)

  if (left > 2) items.push('...')
  for (let p = left; p <= right; p++) items.push(p)
  if (right < totalPages - 1) items.push('...')
  items.push(totalPages)
  return items
}

/**
 * Pagination 组件
 * - 以简洁紧凑的风格呈现列表分页控制
 */
const Pagination: React.FC<PaginationProps> = ({
  total,
  page,
  pageSize,
  onPageChange,
  showSizeChanger = true,
  pageSizeOptions = [10, 20, 50, 100],
  onPageSizeChange,
  className,
}) => {
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const pageItems = useMemo(() => getPageItems(totalPages, Math.min(page, totalPages)), [totalPages, page])

  const canPrev = page > 1
  const canNext = page < totalPages

  return (
    <div
      className={['flex flex-col items-center gap-3 sm:flex-row sm:justify-between', className || ''].join(' ')}
      aria-label="分页"
    >
      {/* 左侧：统计信息 */}
      <div className="text-sm text-muted-foreground">
        共 {total} 条 · 第 {total === 0 ? 0 : (page - 1) * pageSize + 1}-
        {Math.min(page * pageSize, total)} 条
      </div>

      {/* 右侧：分页控制 */}
      <div className="flex flex-wrap items-center gap-2">
        {/* 页大小选择 */}
        {showSizeChanger && onPageSizeChange && (
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">每页</span>
            <ModernSelect
              value={pageSize.toString()}
              onValueChange={(value) => onPageSizeChange(Number(value))}
              options={pageSizeOptions.map(opt => ({
                value: opt.toString(),
                label: opt.toString()
              }))}
              triggerClassName="h-8 w-16"
            />
          </div>
        )}

        {/* 分页按钮 */}
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            className="bg-transparent h-8 px-2"
            onClick={() => onPageChange(1)}
            disabled={!canPrev}
            aria-label="首页"
            title="首页"
          >
            <ChevronsLeft size={16} />
          </Button>
          <Button
            variant="outline"
            className="bg-transparent h-8 px-2"
            onClick={() => onPageChange(page - 1)}
            disabled={!canPrev}
            aria-label="上一页"
            title="上一页"
          >
            <ChevronLeft size={16} />
          </Button>

          {pageItems.map((it, idx) =>
            it === '...' ? (
              <span key={`e-${idx}`} className="px-2 text-sm text-muted-foreground select-none">
                ...
              </span>
            ) : (
              <Button
                key={it}
                variant="outline"
                className={[
                  'bg-transparent h-8 px-2',
                  it === page ? 'border-primary text-primary' : '',
                ].join(' ')}
                onClick={() => onPageChange(it)}
                aria-current={it === page ? 'page' : undefined}
              >
                {it}
              </Button>
            ),
          )}

          <Button
            variant="outline"
            className="bg-transparent h-8 px-2"
            onClick={() => onPageChange(page + 1)}
            disabled={!canNext}
            aria-label="下一页"
            title="下一页"
          >
            <ChevronRight size={16} />
          </Button>
          <Button
            variant="outline"
            className="bg-transparent h-8 px-2"
            onClick={() => onPageChange(totalPages)}
            disabled={!canNext}
            aria-label="末页"
            title="末页"
          >
            <ChevronsRight size={16} />
          </Button>
        </div>
      </div>
    </div>
  )
}

export default Pagination
