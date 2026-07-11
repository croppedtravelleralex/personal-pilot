export type DirectProxyProtocol = 'http' | 'https' | 'socks5'

export interface DirectImportForm {
  proxyName: string
  protocol: DirectProxyProtocol
  server: string
  port: string
  username: string
  password: string
}

export interface ImportCandidate {
  proxyName: string
  proxyConfig: string
}

interface DirectProxyLineParts {
  server: string
  port: string
  username: string
  password: string
  protocol?: DirectProxyProtocol
  query?: string
}

function normalizeDirectProxyConfig(raw: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return ''
  if (/^socket:\/\//i.test(trimmed)) {
    return trimmed.replace(/^socket:\/\//i, 'socks5://')
  }
  if (/^socks:\/\//i.test(trimmed)) {
    return trimmed.replace(/^socks:\/\//i, 'socks5://')
  }
  return trimmed
}

function resolveDirectProxyName(rawName: string, scheme: string, server: string, port: number, index: number, prefix: string): string {
  const name = rawName.trim()
  const fallbackName = server
    ? `${scheme.toUpperCase()}-${server}${port > 0 ? `:${port}` : ''}`
    : `导入代理 ${index + 1}`
  const finalName = name || fallbackName
  return prefix ? `${prefix}-${finalName}` : finalName
}

function formatDirectProxyHost(raw: string): string {
  const host = raw.trim()
  if (!host) return ''
  if (host.startsWith('[') && host.endsWith(']')) {
    return host
  }
  return host.includes(':') ? `[${host}]` : host
}

function isValidDirectProxyPort(raw: string): boolean {
  if (!/^\d+$/.test(raw)) return false
  const port = Number(raw)
  return port >= 1 && port <= 65535
}

function normalizeDirectProxyServer(raw: string): string {
  return raw.trim().replace(/^\[(.*)\]$/, '$1')
}

function scoreDirectProxyServer(raw: string): number {
  const server = normalizeDirectProxyServer(raw)
  if (!server) return -1
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(server)) return 4
  if (server.includes('.')) return 3
  if (server.includes(':')) return 3
  if (server.toLowerCase() === 'localhost') return 2
  if (/^[a-zA-Z0-9-]+$/.test(server) && !/^\d+$/.test(server)) return 1
  return 0
}

function normalizeDirectProxyProtocol(raw: string): DirectProxyProtocol {
  const protocol = raw.toLowerCase()
  if (protocol === 'socks' || protocol === 'socket') return 'socks5'
  if (protocol === 'http' || protocol === 'https' || protocol === 'socks5') return protocol
  return 'http'
}

function buildDirectProxyLineParts(server: string, port: string, username: string, password: string): DirectProxyLineParts | null {
  if (!normalizeDirectProxyServer(server) || !isValidDirectProxyPort(port)) return null
  return {
    server: normalizeDirectProxyServer(server),
    port,
    username,
    password,
  }
}

function chooseDirectProxyLineParts(
  preferred: DirectProxyLineParts | null,
  alternative: DirectProxyLineParts | null,
): DirectProxyLineParts | null {
  if (!preferred) return alternative
  if (!alternative) return preferred
  return scoreDirectProxyServer(alternative.server) > scoreDirectProxyServer(preferred.server)
    ? alternative
    : preferred
}

export function parseDirectProxyLine(raw: string): DirectProxyLineParts | null {
  const input = raw.trim()
  if (!input) return null

  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(input)) {
    try {
      const normalized = normalizeDirectProxyConfig(input)
      const parsedURL = new URL(normalized)
      if (!parsedURL.hostname || !parsedURL.port || !isValidDirectProxyPort(parsedURL.port)) {
        return null
      }
      return {
        server: normalizeDirectProxyServer(parsedURL.hostname),
        port: parsedURL.port,
        username: decodeURIComponent(parsedURL.username || ''),
        password: decodeURIComponent(parsedURL.password || ''),
        protocol: normalizeDirectProxyProtocol(parsedURL.protocol.replace(/:$/, '')),
        query: parsedURL.search,
      }
    } catch {
      return null
    }
  }

  const atIndex = input.indexOf('@')
  if (atIndex > 0 && atIndex < input.length - 1) {
    const left = input.slice(0, atIndex).trim()
    const right = input.slice(atIndex + 1).trim()
    const leftParts = left.split(':').map(part => part.trim())
    const rightParts = right.split(':').map(part => part.trim())
    if (leftParts.length === 2 && rightParts.length === 2) {
      return chooseDirectProxyLineParts(
        buildDirectProxyLineParts(leftParts[0] || '', leftParts[1] || '', rightParts[0] || '', rightParts[1] || ''),
        buildDirectProxyLineParts(rightParts[0] || '', rightParts[1] || '', leftParts[0] || '', leftParts[1] || ''),
      )
    }
    return null
  }

  const parts = input.split(':').map(part => part.trim())
  if (parts.length === 2 && isValidDirectProxyPort(parts[1] || '')) {
    return {
      server: normalizeDirectProxyServer(parts[0] || ''),
      port: parts[1] || '',
      username: '',
      password: '',
    }
  }
  if (parts.length !== 4) return null

  const [first = '', second = '', third = '', fourth = ''] = parts
  return chooseDirectProxyLineParts(
    buildDirectProxyLineParts(first, second, third, fourth),
    buildDirectProxyLineParts(third, fourth, first, second),
  )
}

export function buildDirectImportCandidate(form: DirectImportForm): ImportCandidate {
  const lineParts = parseDirectProxyLine(form.server)
  const serverInput = (lineParts?.server || form.server).trim()
  if (!serverInput) {
    throw new Error('请输入代理地址')
  }
  if (!lineParts && /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(serverInput)) {
    throw new Error('代理地址只需要填写主机名或 IP，不需要协议头')
  }

  const portInput = (lineParts?.port || form.port).trim()
  if (!portInput) {
    throw new Error('请输入代理端口')
  }
  if (!/^\d+$/.test(portInput)) {
    throw new Error('代理端口必须为数字')
  }

  const port = Number(portInput)
  if (port < 1 || port > 65535) {
    throw new Error('代理端口必须在 1-65535 之间')
  }

  const username = (form.username.trim() || lineParts?.username || '').trim()
  const password = form.password || lineParts?.password || ''
  if (password && !username) {
    throw new Error('填写密码时请同时填写账号')
  }

  const protocol = lineParts?.protocol || form.protocol
  const auth = username
    ? `${encodeURIComponent(username)}${password ? `:${encodeURIComponent(password)}` : ''}@`
    : ''
  const rawConfig = `${protocol}://${auth}${formatDirectProxyHost(serverInput)}:${port}`

  let parsedURL: URL
  try {
    parsedURL = new URL(rawConfig)
  } catch {
    throw new Error('请输入有效的代理地址')
  }

  if (!parsedURL.hostname) {
    throw new Error('请输入有效的代理地址')
  }
  if (lineParts?.query) {
    parsedURL.search = lineParts.query
  }

  const normalizedConfig = normalizeDirectProxyConfig(parsedURL.toString())
    .replace(/\/(?=\?)/, '')
    .replace(/\/$/, '')
  const normalizedServer = parsedURL.hostname.replace(/^\[(.*)\]$/, '$1')

  return {
    proxyName: resolveDirectProxyName(form.proxyName, protocol, normalizedServer, port, 0, ''),
    proxyConfig: normalizedConfig,
  }
}
