/**
 * ProviderKeysTable.tsx
 * 账号 API Keys 列表表格组件（无外部卡片样式）：
 * - 展示名称、服务商、密钥（脱敏）、权重、限流、状态、创建时间、操作
 * - 交互：启用开关、编辑、删除、统计查看
 * - 外部容器（卡片/边框/阴影）由页面决定，保持全站风格一致
 */

import React from 'react'
import { Switch } from '@/components/ui/switch'
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from '@/components/ui/table'
import { Button } from '@/components/ui/button'
import { BarChart2, Edit2, Trash2 } from 'lucide-react'
import type { AddProviderKeyForm } from './AddProviderKeyDialog'

/** 单条账号Key数据结构（基于表单增加 id 与 createdAt） */
export interface ProviderKeyItem extends AddProviderKeyForm {
  /** 唯一ID */
  id: string
  /** 创建时间ISO字符串 */
  createdAt: string
}

/** 组件属性：数据与回调 */
export interface ProviderKeysTableProps {
  /** 数据源 */
  data: ProviderKeyItem[]
  /** 切换启用状态 */
  onToggleEnabled: (id: string, enabled: boolean) => void
  /** 点击编辑 */
  onEdit: (item: ProviderKeyItem) => void
  /** 点击删除 */
  onDelete: (item: ProviderKeyItem) => void
  /** 点击统计 */
  onShowStats: (item: ProviderKeyItem) => void
}

/** 服务商值到显示文本的映射 */
const providerLabelMap: Record<NonNullable<AddProviderKeyForm['provider']>, string> = {
  openai: 'OpenAI',
  claude: 'Anthropic Claude',
  gemini: 'Google Gemini',
  custom: '自定义',
}

/**
 * 将密钥进行脱敏显示
 */
function maskKey(key: string): string {
  if (!key) return ''
  if (key.length <= 6) return '*'.repeat(Math.max(4, key.length))
  return `${key.slice(0, 3)}***${key.slice(-2)}`
}

/**
 * 日期格式化（到分钟）
 */
function formatDate(iso: string) {
  try {
    const d = new Date(iso)
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, '0')
    const day = String(d.getDate()).padStart(2, '0')
    const hh = String(d.getHours()).padStart(2, '0')
    const mm = String(d.getMinutes()).padStart(2, '0')
    return `${y}-${m}-${day} ${hh}:${mm}`
  } catch {
    return iso
  }
}

/**
 * ProviderKeysTable
 * - 无外层边框/背景，由页面卡片容器统一控制
 */
const ProviderKeysTable: React.FC<ProviderKeysTableProps> = ({
  data,
  onToggleEnabled,
  onEdit,
  onDelete,
  onShowStats,
}) => {
  return (
    <div className="w-full">
      <Table className="min-w-[960px]">
        <TableHeader className="bg-muted/40">
          <TableRow>
            <TableHead className="min-w-[160px]">名称</TableHead>
            <TableHead className="min-w-[140px]">服务商</TableHead>
            <TableHead className="min-w-[200px]">API 密钥</TableHead>
            <TableHead className="min-w-[140px]">项目ID</TableHead>
            <TableHead className="min-w-[80px] text-right">权重</TableHead>
            <TableHead className="min-w-[140px] text-right">请求/分钟</TableHead>
            <TableHead className="min-w-[140px] text-right">Token/天</TableHead>
            <TableHead className="min-w-[120px]">状态</TableHead>
            <TableHead className="min-w-[160px]">创建时间</TableHead>
            <TableHead className="min-w-[220px] text-right">操作</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {data.map((row) => (
            <TableRow key={row.id}>
              <TableCell className="font-medium text-foreground">{row.name}</TableCell>
              <TableCell className="text-foreground/80">
                {row.provider ? providerLabelMap[row.provider] : '-'}
              </TableCell>
              <TableCell>
                <span className="rounded bg-muted px-2 py-0.5 text-sm text-foreground/70">
                  {maskKey(row.apiKey)}
                </span>
              </TableCell>
              <TableCell className="text-foreground/70">
                {row.projectId || '-'}
              </TableCell>
              <TableCell className="text-right tabular-nums">{row.weight}</TableCell>
              <TableCell className="text-right tabular-nums">{row.rateLimitPerMin}</TableCell>
              <TableCell className="text-right tabular-nums">{row.tokenLimitPerDay}</TableCell>
              <TableCell>
                <div className="flex items-center gap-2">
                  <span
                    className={[
                      'inline-flex items-center rounded-full px-2 py-0.5 text-xs',
                      row.enabled ? 'bg-emerald-50 text-emerald-600' : 'bg-muted text-muted-foreground',
                    ].join(' ')}
                  >
                    {row.enabled ? '启用' : '禁用'}
                  </span>
                  <Switch checked={row.enabled} onCheckedChange={(v) => onToggleEnabled(row.id, v)} />
                </div>
              </TableCell>
              <TableCell className="text-foreground/70">{formatDate(row.createdAt)}</TableCell>
              <TableCell className="text-right">
                <div className="flex items-center justify-end gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    className="bg-transparent"
                    onClick={() => onShowStats(row)}
                  >
                    <BarChart2 className="mr-1 h-4 w-4" /> 统计
                  </Button>
                  <Button size="sm" variant="outline" className="bg-transparent" onClick={() => onEdit(row)}>
                    <Edit2 className="mr-1 h-4 w-4" /> 编辑
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="bg-transparent text-red-600 hover:text-red-700"
                    onClick={() => onDelete(row)}
                  >
                    <Trash2 className="mr-1 h-4 w-4" /> 删除
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  )
}

export default ProviderKeysTable
