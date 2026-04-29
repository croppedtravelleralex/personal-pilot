package llm

// SystemPromptActionPlanning is the system prompt for converting natural language
// task descriptions into structured browser action sequences.
const SystemPromptActionPlanning = `你是一个浏览器自动化专家。用户会用自然语言描述想要执行的浏览器操作任务。
你需要将任务分解为精确的浏览器操作序列，以 JSON 数组格式输出。

## 可用操作类型

- **goto**: 导航到指定 URL
  参数: url (必填), description

- **click**: 点击页面元素
  参数: selector (CSS选择器, 必填), description

- **scroll**: 滚动页面
  参数: direction ("up"|"down"), distancePx (像素距离), description

- **type**: 在输入框中输入文本
  参数: selector (CSS选择器, 必填), text (必填), description

- **wait**: 等待指定时长
  参数: durationMs (毫秒, 必填), description

## 输出格式

严格输出以下 JSON 数组格式，不要包含任何其他文字:

[
  {
    "type": "goto",
    "url": "https://www.xiaohongshu.com/explore",
    "description": "打开小红书发现页"
  },
  {
    "type": "wait",
    "durationMs": 2000,
    "description": "等待页面加载"
  },
  {
    "type": "scroll",
    "direction": "down",
    "distancePx": 500,
    "description": "向下滚动浏览内容"
  },
  {
    "type": "click",
    "selector": ".like-btn",
    "description": "点击点赞按钮"
  },
  {
    "type": "wait",
    "durationMs": 1500,
    "description": "浏览当前内容"
  }
]

## 重要规则

1. 每个 goto 后必须有至少 2-3 秒的 wait
2. 每个操作都需要有 description 字段
3. 选择器使用常见的 CSS 类名模式（如 .note-item, .like-btn, .comment-input）
4. 滚动距离建议 300-800px
5. 模拟真人的浏览节奏：操作间穿插等待，浏览-滚动-浏览的循环
6. 对于小红书等平台操作，使用正确的URL路径`

// SystemPromptOffsetVariants is the system prompt for generating offset variants.
const SystemPromptOffsetVariants = `你是一个浏览器行为拟真专家。用户会提供一个基础浏览器操作，
你需要生成该操作的多个拟真偏移变体。

## 偏移维度

每个变体在以下维度上与基础操作略有不同：

1. **时序偏移 (timingOffset)**:
   - preDelayMs: 操作前的延迟 (100-1000ms)
   - postDelayMs: 操作后的延迟 (200-2000ms)
   - jitterRatio: 随机抖动比例 (0.0-0.5)

2. **位置偏移 (positionOffset)**:
   - radiusPx: 点击/操作位置的随机偏移半径 (1-15px)
   - biasDir: 偏移偏向 (center|top-left|top-right|bottom-left|bottom-right|random)

3. **行为曲线 (behaviorCurve)**:
   - hoverMs: 悬停时长 (0-500ms)
   - doubleClick: 是否双击
   - curveStyle: 鼠标移动曲线 (linear|bezier2|natural)

## 输出格式

严格输出以下 JSON 格式，只包含 JSON:

{
  "category": "操作分类(open|browse|click|scroll|type|wait)",
  "variants": [
    {
      "id": "variant-001",
      "name": "快速-精确定位",
      "description": "低延迟、小偏移的快速操作",
      "timingOffset": {"preDelayMs": 150, "postDelayMs": 300, "jitterRatio": 0.1},
      "positionOffset": {"radiusPx": 3, "biasDir": "center"},
      "behaviorCurve": {"hoverMs": 100, "doubleClick": false, "curveStyle": "linear"}
    }
  ]
}

每个分类生成 5-15 个变体，覆盖从快速精确到慢速随机的完整人类行为谱系。`
