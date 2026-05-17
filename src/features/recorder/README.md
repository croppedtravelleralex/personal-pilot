# Recorder Feature

负责“录制行为作为模板”的录制状态机与步骤捕获，不负责最终运行编译。

建议内部继续拆成：

- `store.ts`
  - recorder session
  - current capture state
  - step timeline
- `hooks.ts`
  - start / stop recording
  - edit / reorder / delete step
- `model.ts`
  - recorded step
  - recorder session
  - step action type

二级子模块边界：

- Recorder Session
- Step Capture
- Step Timeline
- Sensitive Data Guard

预期依赖的桌面契约：

- `startBehaviorRecording`
- `stopBehaviorRecording`
- `readRecorderSnapshot`
