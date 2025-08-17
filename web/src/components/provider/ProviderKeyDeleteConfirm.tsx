/**
 * ProviderKeyDeleteConfirm.tsx
 * 删除账号 API Key 的二次确认弹窗。
 */

import React from 'react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'

/** 组件属性 */
export interface ProviderKeyDeleteConfirmProps {
  /** 是否打开 */
  open: boolean
  /** 打开状态变更 */
  onOpenChange: (open: boolean) => void
  /** 待删除名称（展示用） */
  name?: string
  /** 确认删除回调 */
  onConfirm: () => void
}

/**
 * ProviderKeyDeleteConfirm
 * - 简单的确认对话框，提示不可恢复。
 */
const ProviderKeyDeleteConfirm: React.FC<ProviderKeyDeleteConfirmProps> = ({
  open,
  onOpenChange,
  name,
  onConfirm,
}) => {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>确认删除</AlertDialogTitle>
          <AlertDialogDescription>
            确认要删除
            <span className="mx-1 font-medium text-foreground">{name || '该密钥'}</span>
            吗？该操作不可恢复，请谨慎操作。
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>取消</AlertDialogCancel>
          <AlertDialogAction className="bg-red-600 hover:bg-red-700" onClick={onConfirm}>
            删除
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export default ProviderKeyDeleteConfirm
