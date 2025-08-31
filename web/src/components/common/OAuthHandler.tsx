/**
 * OAuthHandler.tsx
 * OAuth授权处理组件 - 处理OAuth弹窗和postMessage通信
 */

import React, { useCallback, useRef } from 'react'
import { ExternalLink, Shield, AlertCircle, CheckCircle2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { api, OAuthAuthorizeRequest, OAuthCallbackResponse } from '@/lib/api'
import { toast } from 'sonner'

/** OAuth状态类型 */
export type OAuthStatus = 'idle' | 'authorizing' | 'waiting' | 'success' | 'error' | 'cancelled'

/** OAuth结果 */
export interface OAuthResult {
  success: boolean
  data?: OAuthCallbackResponse
  error?: string
  cancelled?: boolean
}

/** 组件Props */
export interface OAuthHandlerProps {
  /** OAuth请求参数 */
  request: OAuthAuthorizeRequest
  /** 当前OAuth状态 */
  status: OAuthStatus
  /** 状态变更回调 */
  onStatusChange: (status: OAuthStatus) => void
  /** OAuth完成回调 */
  onComplete: (result: OAuthResult) => void
  /** 是否禁用 */
  disabled?: boolean
  /** 自定义样式类名 */
  className?: string
  /** 按钮文本 */
  buttonText?: string
  /** 按钮变体 */
  buttonVariant?: 'default' | 'outline' | 'secondary'
}

/**
 * OAuthHandler OAuth授权处理器
 * - 启动OAuth授权流程
 * - 管理OAuth弹窗
 * - 处理postMessage通信
 * - 监听授权结果
 */
const OAuthHandler: React.FC<OAuthHandlerProps> = ({
  request,
  status,
  onStatusChange,
  onComplete,
  disabled = false,
  className,
  buttonText = '开始OAuth授权',
  buttonVariant = 'default',
}) => {
  const popupRef = useRef<Window | null>(null)
  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null)
  const messageListenerRef = useRef<((event: MessageEvent) => void) | null>(null)

  /** 清理资源 */
  const cleanup = useCallback(() => {
    // 关闭弹窗
    if (popupRef.current && !popupRef.current.closed) {
      popupRef.current.close()
      popupRef.current = null
    }

    // 清理轮询定时器
    if (pollIntervalRef.current) {
      clearInterval(pollIntervalRef.current)
      pollIntervalRef.current = null
    }

    // 移除消息监听器
    if (messageListenerRef.current) {
      window.removeEventListener('message', messageListenerRef.current)
      messageListenerRef.current = null
    }
  }, [])

  /** 启动OAuth授权流程 */
  const startOAuthFlow = useCallback(async () => {
    if (status !== 'idle' || disabled) return

    try {
      onStatusChange('authorizing')
      
      // 调用后端API启动OAuth流程
      const response = await api.auth.initiateOAuth(request)
      
      if (!response.success || !response.data) {
        throw new Error(response.error?.message || 'OAuth授权启动失败')
      }

      const { authorization_url, session_id } = response.data

      // 打开OAuth授权弹窗
      const popupFeatures = [
        'width=600',
        'height=700',
        'left=' + Math.round(window.screenX + (window.outerWidth - 600) / 2),
        'top=' + Math.round(window.screenY + (window.outerHeight - 700) / 2.5),
        'toolbar=no',
        'location=no',
        'directories=no',
        'status=no',
        'menubar=no',
        'scrollbars=yes',
        'resizable=yes',
      ].join(',')

      popupRef.current = window.open(authorization_url, 'oauth_popup', popupFeatures)
      
      if (!popupRef.current) {
        throw new Error('无法打开OAuth授权弹窗，请检查弹窗拦截设置')
      }

      onStatusChange('waiting')

      // 监听postMessage消息
      const messageListener = (event: MessageEvent) => {
        // 验证消息来源（这里可以根据需要添加更严格的验证）
        if (event.origin !== window.location.origin) {
          return
        }

        const { type, data, error } = event.data

        switch (type) {
          case 'OAUTH_SUCCESS':
            cleanup()
            onStatusChange('success')
            onComplete({
              success: true,
              data: data,
            })
            toast.success('OAuth授权成功！')
            break

          case 'OAUTH_ERROR':
            cleanup()
            onStatusChange('error')
            onComplete({
              success: false,
              error: error?.message || 'OAuth授权失败',
            })
            toast.error(`OAuth授权失败: ${error?.message || '未知错误'}`)
            break

          case 'OAUTH_CANCEL':
            cleanup()
            onStatusChange('cancelled')
            onComplete({
              success: false,
              cancelled: true,
            })
            toast.info('OAuth授权已取消')
            break
        }
      }

      messageListenerRef.current = messageListener
      window.addEventListener('message', messageListener)

      // 轮询检查弹窗状态（防止用户直接关闭弹窗）
      pollIntervalRef.current = setInterval(() => {
        if (popupRef.current?.closed) {
          cleanup()
          onStatusChange('cancelled')
          onComplete({
            success: false,
            cancelled: true,
          })
          toast.info('OAuth授权窗口已关闭')
        }
      }, 1000)

    } catch (error) {
      cleanup()
      onStatusChange('error')
      const errorMessage = error instanceof Error ? error.message : 'OAuth授权启动失败'
      onComplete({
        success: false,
        error: errorMessage,
      })
      toast.error(errorMessage)
    }
  }, [request, status, disabled, onStatusChange, onComplete, cleanup])

  /** 取消OAuth流程 */
  const cancelOAuthFlow = useCallback(() => {
    cleanup()
    onStatusChange('cancelled')
    onComplete({
      success: false,
      cancelled: true,
    })
    toast.info('OAuth授权已取消')
  }, [cleanup, onStatusChange, onComplete])

  /** 重新开始OAuth流程 */
  const retryOAuthFlow = useCallback(() => {
    cleanup()
    onStatusChange('idle')
  }, [cleanup, onStatusChange])

  // 组件卸载时清理资源
  React.useEffect(() => {
    return cleanup
  }, [cleanup])

  /** 渲染状态指示器 */
  const renderStatusIndicator = () => {
    switch (status) {
      case 'authorizing':
        return (
          <Badge variant="secondary" className="flex items-center gap-1">
            <div className="w-2 h-2 bg-blue-500 rounded-full animate-pulse" />
            正在启动授权...
          </Badge>
        )
      
      case 'waiting':
        return (
          <Badge variant="secondary" className="flex items-center gap-1">
            <div className="w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
            等待用户授权
          </Badge>
        )
      
      case 'success':
        return (
          <Badge variant="default" className="flex items-center gap-1 bg-green-500">
            <CheckCircle2 className="h-3 w-3" />
            授权成功
          </Badge>
        )
      
      case 'error':
        return (
          <Badge variant="destructive" className="flex items-center gap-1">
            <AlertCircle className="h-3 w-3" />
            授权失败
          </Badge>
        )
      
      case 'cancelled':
        return (
          <Badge variant="outline" className="flex items-center gap-1">
            <AlertCircle className="h-3 w-3" />
            授权取消
          </Badge>
        )
      
      default:
        return null
    }
  }

  return (
    <Card className={cn('', className)}>
      <CardHeader className="pb-4">
        <CardTitle className="flex items-center gap-2 text-base">
          <Shield className="h-5 w-5" />
          OAuth 2.0 授权
          {renderStatusIndicator()}
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        <div className="text-sm text-muted-foreground">
          <p>将打开新窗口进行OAuth授权，请在弹出窗口中完成授权流程。</p>
          {status === 'waiting' && (
            <p className="mt-2 text-blue-600 dark:text-blue-400">
              💡 授权窗口已打开，请在弹窗中完成授权操作
            </p>
          )}
        </div>

        <div className="flex gap-2">
          {status === 'idle' || status === 'error' || status === 'cancelled' ? (
            <Button
              onClick={status === 'idle' ? startOAuthFlow : retryOAuthFlow}
              disabled={disabled}
              variant={buttonVariant}
              className="flex items-center gap-2"
            >
              <ExternalLink className="h-4 w-4" />
              {status === 'idle' ? buttonText : '重新授权'}
            </Button>
          ) : status === 'waiting' ? (
            <Button
              onClick={cancelOAuthFlow}
              variant="outline"
              className="flex items-center gap-2"
            >
              <AlertCircle className="h-4 w-4" />
              取消授权
            </Button>
          ) : null}
        </div>

        {/* 安全提示 */}
        <div className="text-xs text-muted-foreground bg-muted/50 p-3 rounded-md">
          🔐 <strong>安全提示：</strong>
          OAuth授权过程中不会要求您输入密码到我们的系统，
          所有授权操作都在官方授权服务器上完成，确保您的账户安全。
        </div>
      </CardContent>
    </Card>
  )
}

export default OAuthHandler