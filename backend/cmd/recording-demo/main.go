// 行为录制/回放 独立命令行演示工具
// 用法:
//
//	record  <debugPort>              开始录制（按 Enter 停止）
//	list                              列出所有已保存录制
//	play   <recordingId> <debugPort>  回放录制（Ctrl+C 停止）
//	inspect <recordingId>             查看录制事件详情
//	delete  <recordingId>             删除录制
//
// 示例:
//
//	go run ./cmd/recording-demo record 9222
//	go run ./cmd/recording-demo list
//	go run ./cmd/recording-demo play rec-xxx 9222
//	go run ./cmd/recording-demo inspect rec-xxx
//	go run ./cmd/recording-demo delete rec-xxx
package main

import (
	"bufio"
	"context"
	"fmt"
	"math"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"time"

	"personal-pilot/backend/internal/behavior"
)

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	store, err := behavior.NewFileRecordingStore("data/recordings")
	if err != nil {
		fmt.Fprintf(os.Stderr, "初始化存储失败: %v\n", err)
		os.Exit(1)
	}

	cmd := os.Args[1]
	switch cmd {
	case "record":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "用法: recording-demo record <debugPort>")
			os.Exit(1)
		}
		port, err := strconv.Atoi(os.Args[2])
		if err != nil {
			fmt.Fprintf(os.Stderr, "无效的端口号: %s\n", os.Args[2])
			os.Exit(1)
		}
		handleRecord(store, port)

	case "list":
		handleList(store)

	case "play":
		if len(os.Args) < 4 {
			fmt.Fprintln(os.Stderr, "用法: recording-demo play <recordingId> <debugPort>")
			os.Exit(1)
		}
		port, err := strconv.Atoi(os.Args[3])
		if err != nil {
			fmt.Fprintf(os.Stderr, "无效的端口号: %s\n", os.Args[3])
			os.Exit(1)
		}
		handlePlay(store, os.Args[2], port)

	case "inspect":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "用法: recording-demo inspect <recordingId>")
			os.Exit(1)
		}
		handleInspect(store, os.Args[2])

	case "delete":
		if len(os.Args) < 3 {
			fmt.Fprintln(os.Stderr, "用法: recording-demo delete <recordingId>")
			os.Exit(1)
		}
		handleDelete(store, os.Args[2])

	default:
		fmt.Fprintf(os.Stderr, "未知命令: %s\n\n", cmd)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Println(`行为录制/回放 命令行工具
───────────────────────────────────────
用法:
  record  <debugPort>              开始录制（按 Enter 停止并保存）
  list                              列出所有已保存录制
  play    <recordingId> <debugPort> 回放录制到指定浏览器
  inspect <recordingId>             查看录制事件详情
  delete  <recordingId>             删除录制

示例:
  recording-demo record 9222        # 录制端口 9222 的浏览器操作
  recording-demo list               # 列出所有录制
  recording-demo play rec-xxx 9222  # 回放到端口 9222
  recording-demo inspect rec-xxx    # 查看 rec-xxx 的事件内容
  recording-demo delete rec-xxx     # 删除 rec-xxx`)
}

// ─── record ────────────────────────────────────────────────

func handleRecord(store *behavior.FileRecordingStore, debugPort int) {
	fmt.Printf("正在连接到浏览器 CDP (端口 %d)...\n", debugPort)

	rec := behavior.NewRecorder()
	if err := rec.StartRecording(debugPort); err != nil {
		fmt.Fprintf(os.Stderr, "启动录制失败: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("✅ 录制已开始！请在浏览器中操作...")
	fmt.Println("   （在浏览器中移动鼠标、点击、输入、滚动等）")
	fmt.Println()
	fmt.Print("按 Enter 停止录制并保存...")

	bufio.NewReader(os.Stdin).ReadString('\n')

	fmt.Println("\n正在停止录制并取回事件数据...")
	recording, err := rec.StopRecording(fmt.Sprintf("录制 %s", time.Now().Format("15:04:05")))
	if err != nil {
		fmt.Fprintf(os.Stderr, "停止录制失败: %v\n", err)
		os.Exit(1)
	}

	if err := store.Save(recording); err != nil {
		fmt.Fprintf(os.Stderr, "保存录制失败: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("✅ 录制已保存!\n")
	fmt.Printf("   ID:       %s\n", recording.ID)
	fmt.Printf("   名称:     %s\n", recording.Name)
	fmt.Printf("   时长:     %.1f 秒\n", float64(recording.DurationMs)/1000)
	fmt.Printf("   事件数:   %d\n", len(recording.Events))
	fmt.Printf("   分辨率:   %dx%d\n", recording.ViewportW, recording.ViewportH)

	printEventStats(recording)
}

// ─── list ──────────────────────────────────────────────────

func handleList(store *behavior.FileRecordingStore) {
	recordings, err := store.List()
	if err != nil {
		fmt.Fprintf(os.Stderr, "列出录制失败: %v\n", err)
		os.Exit(1)
	}

	if len(recordings) == 0 {
		fmt.Println("没有已保存的录制")
		fmt.Println("用法: recording-demo record <debugPort>  创建第一个录制")
		return
	}

	fmt.Printf("已保存录制: %d 个\n", len(recordings))
	fmt.Println(strings.Repeat("─", 80))
	for _, r := range recordings {
		fmt.Printf("ID:       %s\n", r.ID)
		fmt.Printf("名称:     %s\n", r.Name)
		fmt.Printf("时长:     %.1fs  |  事件: %d  |  分辨率: %dx%d  |  创建: %s\n",
			float64(r.DurationMs)/1000,
			len(r.Events),
			r.ViewportW, r.ViewportH,
			r.CreatedAt,
		)
		fmt.Println(strings.Repeat("─", 80))
	}
}

// ─── play ──────────────────────────────────────────────────

func handlePlay(store *behavior.FileRecordingStore, recordingID string, debugPort int) {
	recording, err := store.Get(recordingID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "获取录制失败: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("录制: %s\n", recording.Name)
	fmt.Printf("事件: %d  |  时长: %.1fs  |  分辨率: %dx%d\n",
		len(recording.Events),
		float64(recording.DurationMs)/1000,
		recording.ViewportW, recording.ViewportH,
	)
	fmt.Println()

	// 默认偏移配置 (30% 强度)
	variation := behavior.VariationConfig{
		Intensity:        0.3,
		TimingJitter:     200,
		PositionJitter:   5,
		SpeedVariation:   0.2,
		MicroCorrections: true,
		ExtraPauses:      true,
	}

	fmt.Println("偏移配置:")
	fmt.Printf("  强度: %d%%  |  时序抖动: %.0fms  |  位置抖动: %.0fpx\n",
		int(variation.Intensity*100), variation.TimingJitter, variation.PositionJitter)
	fmt.Printf("  速度变化: %d%%  |  微修正: %v  |  额外停顿: %v\n",
		int(variation.SpeedVariation*100), variation.MicroCorrections, variation.ExtraPauses)
	fmt.Println()

	fmt.Printf("正在连接到浏览器 CDP (端口 %d)...\n", debugPort)
	fmt.Println("▶️  回放已开始！（按 Ctrl+C 停止）")
	fmt.Println()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// 捕获 Ctrl+C
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt)

	engine := behavior.NewPlaybackEngine(recording, variation)

	go func() {
		<-sigCh
		fmt.Println("\n⏹  正在停止回放...")
		engine.Stop()
		cancel()
	}()

	startTime := time.Now()
	if err := engine.Play(ctx, debugPort); err != nil && err != context.Canceled {
		fmt.Fprintf(os.Stderr, "回放出错: %v\n", err)
		os.Exit(1)
	}

	elapsed := time.Since(startTime)
	fmt.Printf("\n✅ 回放完成 (耗时 %.1fs)\n", elapsed.Seconds())
}

// ─── inspect ────────────────────────────────────────────────

func handleInspect(store *behavior.FileRecordingStore, recordingID string) {
	recording, err := store.Get(recordingID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "获取录制失败: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("录制详情: %s\n", recording.Name)
	fmt.Printf("ID:       %s\n", recording.ID)
	fmt.Printf("时长:     %.1fs  |  事件: %d  |  分辨率: %dx%d  |  创建: %s\n",
		float64(recording.DurationMs)/1000,
		len(recording.Events),
		recording.ViewportW, recording.ViewportH,
		recording.CreatedAt,
	)
	fmt.Println(strings.Repeat("─", 80))

	printEventStats(recording)
	fmt.Println()

	// 打印前 20 个事件作为预览
	previewCount := len(recording.Events)
	if previewCount > 20 {
		previewCount = 20
	}
	fmt.Printf("事件预览 (前 %d 个):\n", previewCount)
	fmt.Println(strings.Repeat("─", 80))
	for i := 0; i < previewCount; i++ {
		e := recording.Events[i]
		switch e.Type {
		case "move":
			fmt.Printf("%4d  [move  ] t=%6dms  (%.0f, %.0f)\n", i+1, e.T, e.X, e.Y)
		case "down":
			fmt.Printf("%4d  [down  ] t=%6dms  (%.0f, %.0f)  btn=%d\n", i+1, e.T, e.X, e.Y, e.Button)
		case "up":
			fmt.Printf("%4d  [up    ] t=%6dms  (%.0f, %.0f)  btn=%d\n", i+1, e.T, e.X, e.Y, e.Button)
		case "click":
			fmt.Printf("%4d  [click ] t=%6dms  (%.0f, %.0f)  btn=%d\n", i+1, e.T, e.X, e.Y, e.Button)
		case "key":
			detail := e.Key
			if e.Text != "" {
				detail += " text=" + e.Text
			}
			fmt.Printf("%4d  [key   ] t=%6dms  %s\n", i+1, e.T, detail)
		case "scroll":
			fmt.Printf("%4d  [scroll] t=%6dms  dx=%.0f  dy=%.0f\n", i+1, e.T, e.DeltaX, e.DeltaY)
		default:
			fmt.Printf("%4d  [%s] t=%6dms\n", i+1, e.Type, e.T)
		}
	}
	if len(recording.Events) > 20 {
		fmt.Printf("... 还有 %d 个事件未显示\n", len(recording.Events)-20)
	}
}

// ─── delete ─────────────────────────────────────────────────

func handleDelete(store *behavior.FileRecordingStore, recordingID string) {
	r, err := store.Get(recordingID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "获取录制失败: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("确认删除录制?\n")
	fmt.Printf("  名称: %s  |  事件: %d  |  时长: %.1fs\n",
		r.Name, len(r.Events), float64(r.DurationMs)/1000)
	fmt.Print("输入 'yes' 确认: ")

	reader := bufio.NewReader(os.Stdin)
	input, _ := reader.ReadString('\n')
	input = strings.TrimSpace(strings.ToLower(input))

	if input != "yes" && input != "y" {
		fmt.Println("已取消")
		return
	}

	if err := store.Delete(recordingID); err != nil {
		fmt.Fprintf(os.Stderr, "删除失败: %v\n", err)
		os.Exit(1)
	}
	fmt.Println("✅ 已删除")
}

// ─── helpers ────────────────────────────────────────────────

func printEventStats(r *behavior.Recording) {
	counts := make(map[string]int)
	for _, e := range r.Events {
		counts[e.Type]++
	}

	fmt.Println("   事件分布:")
	for _, t := range []string{"move", "down", "up", "click", "key", "scroll"} {
		if c, ok := counts[t]; ok {
			fmt.Printf("     %-8s %d\n", t+":", c)
		}
	}

	// 计算移动距离
	var totalMove float64
	var lastX, lastY float64
	moveCount := 0
	for _, e := range r.Events {
		if e.Type == "move" {
			if moveCount > 0 {
				totalMove += math.Sqrt((e.X-lastX)*(e.X-lastX) + (e.Y-lastY)*(e.Y-lastY))
			}
			lastX, lastY = e.X, e.Y
			moveCount++
		}
	}
	if totalMove > 0 {
		fmt.Printf("     %-8s %.0f px\n", "移动距离:", totalMove)
	}
}
