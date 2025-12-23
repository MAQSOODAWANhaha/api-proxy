const IS_DEV = typeof __DEV__ !== 'undefined' ? __DEV__ : true

const SENSITIVE_KEYS = new Set([
  'token',
  'access_token',
  'refresh_token',
  'authorization',
  'api_key',
  'key',
  'keyValue',
  'session_id',
])

const redactValue = (value: unknown): unknown => {
  if (value === null || value === undefined) return value
  if (typeof value !== 'object') return value
  try {
    return JSON.parse(
      JSON.stringify(value, (key, val) =>
        SENSITIVE_KEYS.has(key) ? '[REDACTED]' : val
      )
    )
  } catch {
    return value
  }
}

const withRedaction = (args: unknown[]) => args.map((arg) => redactValue(arg))

export const logger = {
  debug: (...args: unknown[]) => {
    if (!IS_DEV) return
    console.log(...withRedaction(args))
  },
  info: (...args: unknown[]) => {
    if (!IS_DEV) return
    console.info(...withRedaction(args))
  },
  warn: (...args: unknown[]) => {
    console.warn(...withRedaction(args))
  },
  error: (...args: unknown[]) => {
    console.error(...withRedaction(args))
  },
}
