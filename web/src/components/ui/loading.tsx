import { cn } from '@/lib/utils'

type LoadingSize = 'sm' | 'md' | 'lg'
type LoadingTone = 'primary' | 'neutral' | 'muted' | 'inverse'

const sizeClasses: Record<LoadingSize, string> = {
  sm: 'h-4 w-4 border-[2px]',
  md: 'h-5 w-5 border-[2px]',
  lg: 'h-8 w-8 border-[3px]',
}

const toneClasses: Record<LoadingTone, string> = {
  primary: 'text-violet-600',
  neutral: 'text-neutral-500',
  muted: 'text-neutral-400',
  inverse: 'text-white',
}

type LoadingSpinnerProps = React.HTMLAttributes<HTMLSpanElement> & {
  size?: LoadingSize
  tone?: LoadingTone
}

export function LoadingSpinner({
  size = 'md',
  tone = 'neutral',
  className,
  ...props
}: LoadingSpinnerProps) {
  return (
    <span
      className={cn(
        'inline-flex animate-spin rounded-full border-current border-r-transparent border-t-transparent motion-reduce:animate-none',
        sizeClasses[size],
        toneClasses[tone],
        className,
      )}
      {...props}
    />
  )
}

type LoadingStateProps = React.HTMLAttributes<HTMLDivElement> & {
  text?: string
  size?: LoadingSize
  tone?: LoadingTone
}

export function LoadingState({
  text = '加载中...',
  size = 'md',
  tone = 'muted',
  className,
  ...props
}: LoadingStateProps) {
  return (
    <div className={cn('flex items-center gap-2 text-sm text-neutral-500', className)} {...props}>
      <LoadingSpinner size={size} tone={tone} />
      <span>{text}</span>
    </div>
  )
}
