// Static behavior preset profiles mirroring backend/internal/behavior/presets.go
// These are the full offset configurations for the 6 built-in behavior personas.

export interface MousePreset {
  enabled: boolean
  idleMoveInterval: string   // e.g. "5s-20s" range
  curveStyle: string          // "bezier2" | "bezier3" | "natural"
  speedMean: number           // px/s
  speedStdDev: number         // px/s
  jitterPx: number            // 1-5 px noise
  pauseProb: number           // 0.0-0.3
  pauseMaxMs: number          // 50-500 ms
}

export interface KeyboardPreset {
  enabled: boolean
  baseDelayMs: number         // 50-200 ms
  delayStdDevMs: number       // 20-80 ms
  burstProb: number           // 0-0.15
  burstKeys: number           // 2-4
  typoProb: number            // 0-0.03
  bigramDelays: boolean
}

export interface ScrollPreset {
  enabled: boolean
  idleScrollProb: number      // 0-0.3
  scrollStepPx: number        // 60-250
  scrollStepStdDev: number    // 25-100
  pauseBetweenMs: number      // 200-2000
  pauseStdDevMs: number       // 100-1000
  overscrollProb: number      // 0-0.15
  reverseProb: number         // 0-0.08
}

export interface IdlePreset {
  mouseWander: boolean
  tabSwitch: boolean
  focusLoss: boolean
}

export interface BehaviorPresetFull {
  id: string
  name: string
  description: string
  mouse: MousePreset
  keyboard: KeyboardPreset
  scroll: ScrollPreset
  idle: IdlePreset
}

export const BEHAVIOR_PRESETS: BehaviorPresetFull[] = [
  {
    id: 'office-worker',
    name: '办公室职员',
    description: '中等速度，标准键盘节奏，偶尔走神滚动，最常用',
    mouse: {
      enabled: true,
      idleMoveInterval: '5s-20s',
      curveStyle: 'bezier2',
      speedMean: 400,
      speedStdDev: 100,
      jitterPx: 2,
      pauseProb: 0.10,
      pauseMaxMs: 150,
    },
    keyboard: {
      enabled: true,
      baseDelayMs: 120,
      delayStdDevMs: 40,
      burstProb: 0.05,
      burstKeys: 2,
      typoProb: 0.01,
      bigramDelays: true,
    },
    scroll: {
      enabled: true,
      idleScrollProb: 0.15,
      scrollStepPx: 120,
      scrollStepStdDev: 40,
      pauseBetweenMs: 800,
      pauseStdDevMs: 400,
      overscrollProb: 0.05,
      reverseProb: 0.03,
    },
    idle: { mouseWander: true, tabSwitch: false, focusLoss: false },
  },
  {
    id: 'student',
    name: '学生',
    description: '快速打字，快速滚动，偶尔急躁操作',
    mouse: {
      enabled: true,
      idleMoveInterval: '3s-10s',
      curveStyle: 'bezier2',
      speedMean: 600,
      speedStdDev: 150,
      jitterPx: 3,
      pauseProb: 0.05,
      pauseMaxMs: 80,
    },
    keyboard: {
      enabled: true,
      baseDelayMs: 80,
      delayStdDevMs: 30,
      burstProb: 0.10,
      burstKeys: 3,
      typoProb: 0.02,
      bigramDelays: false,
    },
    scroll: {
      enabled: true,
      idleScrollProb: 0.25,
      scrollStepPx: 200,
      scrollStepStdDev: 80,
      pauseBetweenMs: 400,
      pauseStdDevMs: 200,
      overscrollProb: 0.10,
      reverseProb: 0.05,
    },
    idle: { mouseWander: true, tabSwitch: true, focusLoss: false },
  },
  {
    id: 'senior-executive',
    name: '高管',
    description: '慢速、深思熟虑、极少打字错误，鼠标移动沉稳',
    mouse: {
      enabled: true,
      idleMoveInterval: '10s-30s',
      curveStyle: 'bezier3',
      speedMean: 250,
      speedStdDev: 60,
      jitterPx: 1,
      pauseProb: 0.20,
      pauseMaxMs: 300,
    },
    keyboard: {
      enabled: true,
      baseDelayMs: 180,
      delayStdDevMs: 50,
      burstProb: 0.02,
      burstKeys: 2,
      typoProb: 0.003,
      bigramDelays: true,
    },
    scroll: {
      enabled: true,
      idleScrollProb: 0.08,
      scrollStepPx: 80,
      scrollStepStdDev: 30,
      pauseBetweenMs: 1500,
      pauseStdDevMs: 800,
      overscrollProb: 0.02,
      reverseProb: 0.01,
    },
    idle: { mouseWander: false, tabSwitch: false, focusLoss: true },
  },
  {
    id: 'gamer',
    name: '游戏玩家',
    description: '极快速鼠标移动，精准点击，键盘高速连击',
    mouse: {
      enabled: true,
      idleMoveInterval: '1s-5s',
      curveStyle: 'natural',
      speedMean: 800,
      speedStdDev: 200,
      jitterPx: 1,
      pauseProb: 0.02,
      pauseMaxMs: 50,
    },
    keyboard: {
      enabled: true,
      baseDelayMs: 50,
      delayStdDevMs: 20,
      burstProb: 0.15,
      burstKeys: 4,
      typoProb: 0.005,
      bigramDelays: false,
    },
    scroll: {
      enabled: true,
      idleScrollProb: 0.30,
      scrollStepPx: 250,
      scrollStepStdDev: 100,
      pauseBetweenMs: 200,
      pauseStdDevMs: 100,
      overscrollProb: 0.15,
      reverseProb: 0.08,
    },
    idle: { mouseWander: true, tabSwitch: true, focusLoss: true },
  },
  {
    id: 'elderly',
    name: '年长用户',
    description: '慢速操作，长停顿，偶尔误触，滚动犹豫',
    mouse: {
      enabled: true,
      idleMoveInterval: '15s-45s',
      curveStyle: 'bezier2',
      speedMean: 180,
      speedStdDev: 80,
      jitterPx: 5,
      pauseProb: 0.30,
      pauseMaxMs: 500,
    },
    keyboard: {
      enabled: true,
      baseDelayMs: 200,
      delayStdDevMs: 80,
      burstProb: 0.01,
      burstKeys: 2,
      typoProb: 0.03,
      bigramDelays: true,
    },
    scroll: {
      enabled: true,
      idleScrollProb: 0.05,
      scrollStepPx: 60,
      scrollStepStdDev: 25,
      pauseBetweenMs: 2000,
      pauseStdDevMs: 1000,
      overscrollProb: 0.08,
      reverseProb: 0.04,
    },
    idle: { mouseWander: false, tabSwitch: false, focusLoss: false },
  },
  {
    id: 'bot-minimal',
    name: '隐身/最小注入',
    description: '几乎无行为注入，仅偶尔鼠标微动防止绝对静止检测',
    mouse: {
      enabled: true,
      idleMoveInterval: '30s-90s',
      curveStyle: 'bezier2',
      speedMean: 100,
      speedStdDev: 30,
      jitterPx: 1,
      pauseProb: 0,
      pauseMaxMs: 0,
    },
    keyboard: {
      enabled: false,
      baseDelayMs: 0,
      delayStdDevMs: 0,
      burstProb: 0,
      burstKeys: 0,
      typoProb: 0,
      bigramDelays: false,
    },
    scroll: {
      enabled: false,
      idleScrollProb: 0,
      scrollStepPx: 0,
      scrollStepStdDev: 0,
      pauseBetweenMs: 0,
      pauseStdDevMs: 0,
      overscrollProb: 0,
      reverseProb: 0,
    },
    idle: { mouseWander: true, tabSwitch: false, focusLoss: false },
  },
]
