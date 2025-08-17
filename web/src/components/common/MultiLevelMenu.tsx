/**
 * MultiLevelMenu.tsx
 * 通用两级菜单组件：父/子互斥选中。高亮使用统一主色（#1890ff），未选中常规色（#333）。
 * 支持受控/非受控、父级展开/收起、响应式尺寸与可选图标。
 */

import React, { useEffect, useMemo, useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'

/** 统一主色与默认文字色（可按需从外部覆盖） */
const PRIMARY = '#1890ff'
const DEFAULT_TEXT = '#333'

/** 子菜单项接口 */
export interface SubMenuItem {
  /** 唯一 key */
  key: string
  /** 文本 */
  label: string
  /** 可选图标 */
  icon?: React.ReactNode
  /** 可选禁用 */
  disabled?: boolean
}

/** 父菜单项接口 */
export interface ParentMenuItem {
  /** 唯一 key */
  key: string
  /** 文本 */
  label: string
  /** 可选图标 */
  icon?: React.ReactNode
  /** 子项 */
  children?: SubMenuItem[]
  /** 可选禁用 */
  disabled?: boolean
}

/** 选中结构：父或子 二选一（互斥） */
export type MenuSelection =
  | { level: 'parent'; key: string }
  | { level: 'child'; key: string; parentKey: string }

/** 组件 props */
export interface MultiLevelMenuProps {
  /** 菜单数据（两级） */
  items: ParentMenuItem[]
  /** 受控选中值 */
  value?: MenuSelection | null
  /** 非受控默认选中值 */
  defaultValue?: MenuSelection | null
  /** 选中变化回调 */
  onChange?: (next: MenuSelection) => void
  /** 容器类名 */
  className?: string
  /** 默认展开所有父级（默认 true） */
  defaultExpandAll?: boolean
  /** 紧凑模式（更小的行高与间距） */
  compact?: boolean
}

/**
 * MultiLevelMenu
 * - 父子互斥：commit 时只保留单一选中对象
 * - 默认展开：defaultExpandAll=true，同时当选中子项时自动展开其父
 * - 无障碍：aria-selected/aria-expanded，键盘可聚焦
 */
const MultiLevelMenu: React.FC<MultiLevelMenuProps> = ({
  items,
  value,
  defaultValue = null,
  onChange,
  className,
  defaultExpandAll = true,
  compact = false,
}) => {
  /** 是否受控 */
  const controlled = typeof value !== 'undefined'
  /** 非受控内部选中状态 */
  const [innerSel, setInnerSel] = useState<MenuSelection | null>(defaultValue)
  /** 当前选中（受控优先） */
  const selection = controlled ? value ?? null : innerSel

  /** 展开状态映射：记录每个父级是否展开 */
  const [open, setOpen] = useState<Record<string, boolean>>(() => {
    const map: Record<string, boolean> = {}
    items.forEach((p) => (map[p.key] = defaultExpandAll))
    return map
  })

  /** items 变化时，补齐 open 字典 */
  useEffect(() => {
    setOpen((prev) => {
      const next = { ...prev }
      items.forEach((p) => {
        if (typeof next[p.key] === 'undefined') next[p.key] = defaultExpandAll
      })
      return next
    })
  }, [items, defaultExpandAll])

  /** 若当前为选中子项，则确保父级展开 */
  useEffect(() => {
    if (selection?.level === 'child') {
      setOpen((s) => ({ ...s, [selection.parentKey]: true }))
    }
  }, [selection])

  /** 切换父级展开/收起 */
  const toggleOpen = (parentKey: string) => {
    setOpen((s) => ({ ...s, [parentKey]: !s[parentKey] }))
  }

  /** 统一提交选中（互斥保障） */
  const commit = (next: MenuSelection) => {
    if (!controlled) setInnerSel(next)
    onChange?.(next)
  }

  /** 便捷判断选中态 */
  const isParentActive = (key: string) => selection?.level === 'parent' && selection.key === key
  const isChildActive = (childKey: string) => selection?.level === 'child' && selection.key === childKey

  /** 行样式（根据紧凑/常规） */
  const parentRowBase =
    'flex w-full items-center gap-2 rounded-lg px-3 transition-colors focus:outline-none focus:ring-2 focus:ring-indigo-200'
  const childRowBase =
    'flex w-full items-center gap-2 rounded-lg px-3 transition-colors text-left focus:outline-none focus:ring-2 focus:ring-indigo-200'

  const parentMinH = compact ? 'min-h-[38px]' : 'min-h-[44px]'
  const childMinH = compact ? 'min-h-[34px]' : 'min-h-[38px]'

  /** 容器样式 */
  const containerClass = useMemo(
    () =>
      [
        'w-full max-w-full sm:max-w-72 rounded-xl border border-neutral-200 bg-white p-2',
        'shadow-sm',
        className || '',
      ].join(' '),
    [className],
  )

  return (
    <nav className={containerClass} aria-label="多级菜单">
      <ul className="space-y-1">
        {items.map((p) => {
          const hasChildren = (p.children?.length ?? 0) > 0
          const expanded = open[p.key]
          const parentActive = isParentActive(p.key)

          const parentTextClass = parentActive ? 'text-white' : 'text-[color:var(--menu-text,#333)] hover:bg-neutral-50'
          const parentIconWrap =
            parentActive ? 'bg-white/20 text-white' : 'bg-neutral-100 text-neutral-600'

          return (
            <li key={p.key}>
              <div className="group flex items-center gap-2">
                {/* 父级选择按钮（与展开按钮分离，避免误触） */}
                <button
                  type="button"
                  disabled={p.disabled}
                  onClick={() => commit({ level: 'parent', key: p.key })}
                  className={[parentRowBase, parentMinH, parentTextClass, p.disabled ? 'opacity-50 cursor-not-allowed' : ''].join(' ')}
                  style={parentActive ? { backgroundColor: PRIMARY } : { ['--menu-text' as any]: DEFAULT_TEXT }}
                  aria-selected={parentActive}
                >
                  {/* 父级图标（可选） */}
                  {p.icon && (
                    <span className={['flex h-6 w-6 items-center justify-center rounded-md', parentIconWrap].join(' ')}>
                      {p.icon}
                    </span>
                  )}
                  <span className="truncate text-sm font-medium">{p.label}</span>
                </button>

                {/* 展开/收起按钮（仅在存在子项时显示） */}
                {hasChildren && (
                  <button
                    type="button"
                    aria-label={expanded ? '收起' : '展开'}
                    onClick={() => toggleOpen(p.key)}
                    className="mr-1 rounded-md p-1 text-neutral-500 hover:bg-neutral-100"
                  >
                    {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                  </button>
                )}
              </div>

              {/* 子菜单列表 */}
              {hasChildren && expanded && (
                <ul className="mt-1 space-y-1 pl-9">
                  {p.children!.map((c) => {
                    const childActive = isChildActive(c.key)
                    const childTextClass = childActive
                      ? 'text-white'
                      : 'text-[color:var(--menu-text,#333)] hover:bg-neutral-50'
                    const childIconWrap =
                      childActive ? 'bg-white/20 text-white' : 'bg-neutral-100 text-neutral-600'

                    return (
                      <li key={c.key}>
                        <button
                          type="button"
                          disabled={c.disabled}
                          onClick={() => commit({ level: 'child', key: c.key, parentKey: p.key })}
                          className={[
                            childRowBase,
                            childMinH,
                            childTextClass,
                            c.disabled ? 'opacity-50 cursor-not-allowed' : '',
                          ].join(' ')}
                          style={childActive ? { backgroundColor: PRIMARY } : { ['--menu-text' as any]: DEFAULT_TEXT }}
                          aria-selected={childActive}
                        >
                          {c.icon && (
                            <span className={['flex h-5 w-5 items-center justify-center rounded', childIconWrap].join(' ')}>
                              {c.icon}
                            </span>
                          )}
                          <span className="truncate text-sm">{c.label}</span>
                        </button>
                      </li>
                    )
                  })}
                </ul>
              )}
            </li>
          )
        })}
      </ul>
    </nav>
  )
}

export default MultiLevelMenu

/**
 * 使用示例（受控）：
 *
 * const [sel, setSel] = useState<MenuSelection | null>({ level: 'parent', key: 'dashboard' })
 * const items: ParentMenuItem[] = [
 *   { key: 'dashboard', label: '仪表板', icon: <LayoutDashboard size={16} /> },
 *   {
 *     key: 'api', label: 'API 密钥管理', icon: <KeyRound size={16} />, children: [
 *       { key: 'user-keys', label: '用户API密钥' },
 *       { key: 'provider-keys', label: '服务商密钥' },
 *     ]
 *   },
 * ]
 * <MultiLevelMenu items={items} value={sel} onChange={setSel} />
 */