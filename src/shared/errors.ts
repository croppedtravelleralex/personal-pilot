export function messageFromUnknownError(error: unknown, fallback = ''): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    return error.trim()
  }

  if (error && typeof error === 'object') {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.trim()) {
      return message
    }
    return fallback
  }

  if (typeof error === 'function') {
    return fallback
  }

  const value = String(error ?? '').trim()
  return value || fallback
}
