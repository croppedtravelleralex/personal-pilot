// 指纹参数序列化/反序列化工具

/**
 * 获取系统当前时区
 * @returns IANA 时区标识符，如 "Asia/Shanghai"
 */
export function getSystemTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone
  } catch {
    return 'UTC'
  }
}

export interface FingerprintConfig {
  // 指纹种子（核心）
  seed?: string            // --fingerprint=<seed>  控制所有随机噪声的根种子

  // 基础身份
  brand?: string           // --fingerprint-brand=
  platform?: string        // --fingerprint-platform=
  lang?: string            // --lang=
  timezone?: string        // --timezone=

  // 系统版本与 HTTP 头
  platformVersion?: string   // --fingerprint-platform-version=   e.g. "10.0.0"
  brandVersion?: string      // --fingerprint-brand-version=      e.g. "120.0.0.0"
  acceptLang?: string        // --accept-lang=                   e.g. "zh-CN,zh;q=0.9,en;q=0.8"

  // 反指纹微调 (Chrome 144+)
  disableSpoofing?: string   // --disable-spoofing=  comma-separated: font,audio,canvas,clientrects,gpu

  // 设备指纹扩展
  userAgent?: string           // --user-agent=                   完整 UA 字符串
  deviceScaleFactor?: string   // --force-device-scale-factor=    devicePixelRatio (1.0/1.25/1.5/2.0)
  disableBatteryApi?: boolean  // --disable-battery-api-override  阻止 getBattery() 指纹检测

  // 屏幕与窗口
  resolution?: string      // --window-size=（预设值或 'custom'）
  customResolution?: string // 当 resolution === 'custom' 时使用
  colorDepth?: string      // --fingerprint-color-depth=

  // 硬件信息
  hardwareConcurrency?: string  // --fingerprint-hardware-concurrency=
  deviceMemory?: string         // --fingerprint-device-memory=

  // 渲染指纹
  canvasNoise?: boolean         // --fingerprint-canvas-noise=
  webglVendor?: string          // --fingerprint-webgl-vendor=
  webglRenderer?: string        // --fingerprint-webgl-renderer=
  audioNoise?: boolean          // --fingerprint-audio-noise=

  // 字体
  fonts?: string                // --fingerprint-fonts=

  // 网络与隐私
  webrtcPolicy?: string         // --webrtc-ip-handling-policy=
  doNotTrack?: boolean          // --fingerprint-do-not-track=

  // 媒体设备
  mediaDevices?: string         // --fingerprint-media-devices= (格式: "2,1,0" 摄像头,麦克风,扬声器)

  // 触摸
  touchPoints?: string          // --fingerprint-touch-points=

  unknownArgs?: string[]        // 无法识别的原始参数，原样保留
}

export const PRESET_RESOLUTIONS = ['1920,1080', '1440,900', '1366,768', '2560,1440', '1280,800', '1600,900']

// CLI 参数前缀 → FingerprintConfig 字段映射
export const KEY_MAP: Record<string, keyof FingerprintConfig> = {
  '--fingerprint': 'seed',
  '--fingerprint-brand': 'brand',
  '--fingerprint-platform': 'platform',
  '--lang': 'lang',
  '--timezone': 'timezone',
  '--fingerprint-platform-version': 'platformVersion',
  '--fingerprint-brand-version': 'brandVersion',
  '--accept-lang': 'acceptLang',
  '--disable-spoofing': 'disableSpoofing',
  '--user-agent': 'userAgent',
  '--force-device-scale-factor': 'deviceScaleFactor',
  '--disable-battery-api-override': 'disableBatteryApi',
  '--window-size': 'resolution',
  '--fingerprint-color-depth': 'colorDepth',
  '--fingerprint-hardware-concurrency': 'hardwareConcurrency',
  '--fingerprint-device-memory': 'deviceMemory',
  '--fingerprint-canvas-noise': 'canvasNoise',
  '--fingerprint-webgl-vendor': 'webglVendor',
  '--fingerprint-webgl-renderer': 'webglRenderer',
  '--fingerprint-audio-noise': 'audioNoise',
  '--fingerprint-fonts': 'fonts',
  '--webrtc-ip-handling-policy': 'webrtcPolicy',
  '--fingerprint-do-not-track': 'doNotTrack',
  '--fingerprint-media-devices': 'mediaDevices',
  '--fingerprint-touch-points': 'touchPoints',
}


// FingerprintConfig → string[]
export function serialize(config: FingerprintConfig): string[] {
  const args: string[] = []
  if (config.seed) args.push(`--fingerprint=${config.seed}`)
  if (config.brand) args.push(`--fingerprint-brand=${config.brand}`)
  if (config.platform) args.push(`--fingerprint-platform=${config.platform}`)
  if (config.lang) args.push(`--lang=${config.lang}`)
  if (config.timezone) {
    // 如果是 system，替换为实际系统时区
    const tz = config.timezone === 'system' ? getSystemTimezone() : config.timezone
    args.push(`--timezone=${tz}`)
  }

  if (config.platformVersion) args.push(`--fingerprint-platform-version=${config.platformVersion}`)
  if (config.brandVersion) args.push(`--fingerprint-brand-version=${config.brandVersion}`)
  if (config.acceptLang) args.push(`--accept-lang=${config.acceptLang}`)
  if (config.disableSpoofing) args.push(`--disable-spoofing=${config.disableSpoofing}`)

  if (config.userAgent) args.push(`--user-agent=${config.userAgent}`)
  if (config.deviceScaleFactor) args.push(`--force-device-scale-factor=${config.deviceScaleFactor}`)
  if (config.disableBatteryApi !== undefined) args.push(`--disable-battery-api-override`)

  const res = config.resolution === 'custom' ? config.customResolution : config.resolution
  if (res) args.push(`--window-size=${res}`)

  if (config.hardwareConcurrency) args.push(`--fingerprint-hardware-concurrency=${config.hardwareConcurrency}`)

  if (config.webrtcPolicy) args.push(`--webrtc-ip-handling-policy=${config.webrtcPolicy}`)

  // colorDepth, deviceMemory, canvasNoise, audioNoise, fonts, doNotTrack,
  // mediaDevices, touchPoints, webglVendor, webglRenderer are deprecated —
  // fingerprint-chromium ignores them in favor of seed-based generation.

  return [...args, ...(config.unknownArgs ?? [])]
}

// string[] → FingerprintConfig
export function deserialize(args: string[]): FingerprintConfig {
  const config: FingerprintConfig = { unknownArgs: [] }

  for (const arg of args) {
    // Handle boolean flags without value (e.g. --disable-battery-api-override)
    if (arg === '--disable-battery-api-override') {
      config.disableBatteryApi = true
      continue
    }

    const eqIdx = arg.indexOf('=')
    if (eqIdx === -1) {
      config.unknownArgs!.push(arg)
      continue
    }
    const key = arg.slice(0, eqIdx)
    const val = arg.slice(eqIdx + 1)
    const field = KEY_MAP[key]

    if (!field) {
      config.unknownArgs!.push(arg)
      continue
    }

    if (field === 'canvasNoise' || field === 'audioNoise' || field === 'doNotTrack' || field === 'disableBatteryApi') {
      (config as Record<string, unknown>)[field] = val === 'true'
    } else if (field === 'resolution') {
      if (PRESET_RESOLUTIONS.includes(val)) {
        config.resolution = val
      } else {
        config.resolution = 'custom'
        config.customResolution = val
      }
    } else {
      (config as Record<string, unknown>)[field] = val
    }
  }

  return config
}

// 生成随机指纹种子（32位正整数）
// Generate a cryptographically random 32-bit positive integer seed.
// Uses crypto.getRandomValues for unpredictability (important for fingerprint isolation).
export function randomFingerprintSeed(): string {
  const buf = new Uint32Array(1)
  crypto.getRandomValues(buf)
  // Ensure positive and within int32 range (1 .. 2147483647)
  return String((buf[0] % 2147483647) + 1)
}

// ─── 预设指纹配置 ────────────────────────────────────────────────────────────

export interface FingerprintPreset {
  id: string
  name: string
  description: string
  config: Partial<FingerprintConfig>
}

export const FINGERPRINT_PRESETS: FingerprintPreset[] = [
  {
    id: 'win-chrome-office',
    name: 'Windows / Chrome / 办公',
    description: '模拟国内办公室 Windows 用户，中文环境，1920x1080',
    config: {
      brand: 'Chrome',
      platform: 'windows',
      lang: 'zh-CN',
      timezone: 'Asia/Shanghai',
      platformVersion: '10.0.22631',
      brandVersion: '130.0.0.0',
      acceptLang: 'zh-CN,zh;q=0.9,en;q=0.8',
      resolution: '1920,1080',
      hardwareConcurrency: '8',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'win-chrome-gaming',
    name: 'Windows / Chrome / 游戏主机',
    description: '模拟高配游戏 PC，NVIDIA 显卡，2560x1440',
    config: {
      brand: 'Chrome',
      platform: 'windows',
      lang: 'en-US',
      timezone: 'America/New_York',
      platformVersion: '10.0.22631',
      brandVersion: '133.0.0.0',
      acceptLang: 'en-US,en;q=0.9',
      resolution: '2560,1440',
      hardwareConcurrency: '16',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'mac-chrome-designer',
    name: 'macOS / Chrome / 设计师',
    description: '模拟 Mac 设计师用户，Apple GPU，Retina 分辨率',
    config: {
      brand: 'Chrome',
      platform: 'mac',
      lang: 'zh-CN',
      timezone: 'Asia/Shanghai',
      platformVersion: '15.0.0',
      brandVersion: '133.0.0.0',
      acceptLang: 'zh-CN,zh;q=0.9,en;q=0.8',
      resolution: '2560,1440',
      hardwareConcurrency: '10',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'win-edge-enterprise',
    name: 'Windows / Edge / 企业',
    description: '模拟企业 Windows 用户，Edge 浏览器，标准配置',
    config: {
      brand: 'Edge',
      platform: 'windows',
      lang: 'zh-CN',
      timezone: 'Asia/Shanghai',
      platformVersion: '10.0.22631',
      brandVersion: '125.0.0.0',
      acceptLang: 'zh-CN,zh;q=0.9,en;q=0.8',
      resolution: '1366,768',
      hardwareConcurrency: '4',
      webrtcPolicy: 'default_public_interface_only',
    },
  },
  {
    id: 'win-chrome-us-user',
    name: 'Windows / Chrome / 美国用户',
    description: '模拟美国普通用户，英文环境，AMD 显卡',
    config: {
      brand: 'Chrome',
      platform: 'windows',
      lang: 'en-US',
      timezone: 'America/Los_Angeles',
      platformVersion: '10.0.22631',
      brandVersion: '130.0.0.0',
      acceptLang: 'en-US,en;q=0.9',
      resolution: '1920,1080',
      hardwareConcurrency: '8',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'mac-safari-jp',
    name: 'macOS / Safari / 日本用户',
    description: '模拟日本 Mac 用户，Safari 风格，日语环境',
    config: {
      brand: 'Safari',
      platform: 'mac',
      lang: 'ja-JP',
      timezone: 'Asia/Tokyo',
      platformVersion: '14.0.0',
      brandVersion: '120.0.0.0',
      acceptLang: 'ja-JP,ja;q=0.9,en;q=0.8',
      resolution: '1440,900',
      hardwareConcurrency: '8',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'win-chrome-uk-office',
    name: 'Windows / Chrome / 英国-办公',
    description: '模拟英国办公室 Windows 用户，英文环境 (en-GB)',
    config: {
      brand: 'Chrome',
      platform: 'windows',
      lang: 'en-GB',
      timezone: 'Europe/London',
      platformVersion: '10.0.22631',
      brandVersion: '130.0.0.0',
      acceptLang: 'en-GB,en;q=0.9',
      resolution: '1920,1080',
      hardwareConcurrency: '8',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
  {
    id: 'mac-chrome-us-edu',
    name: 'macOS / Chrome / 美国-教育',
    description: '模拟美国大学教育网 Mac 用户，英文环境 (en-US)',
    config: {
      brand: 'Chrome',
      platform: 'mac',
      lang: 'en-US',
      timezone: 'America/New_York',
      platformVersion: '14.0.0',
      brandVersion: '130.0.0.0',
      acceptLang: 'en-US,en;q=0.9',
      resolution: '1440,900',
      hardwareConcurrency: '8',
      webrtcPolicy: 'disable_non_proxied_udp',
    },
  },
]
