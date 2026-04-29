// 小红书养号 + 50方向偏移 端到端演示
// 用法: go run ./cmd/xhs-nurture-demo <debugPort>
//
//	或: go run ./cmd/xhs-nurture-demo mock  (生成模拟数据，不需要浏览器)
package main

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"time"

	"ant-chrome/backend/internal/behavior"
	"ant-chrome/backend/internal/behavior/offsets"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Println("用法:")
		fmt.Println("  go run ./cmd/xhs-nurture-demo <debugPort>  # 连接真实浏览器执行养号")
		fmt.Println("  go run ./cmd/xhs-nurture-demo mock         # 生成模拟录制+偏移数据")
		os.Exit(1)
	}

	mode := os.Args[1]
	store, err := behavior.NewFileRecordingStore("data/recordings")
	if err != nil {
		fmt.Fprintf(os.Stderr, "初始化录制存储失败: %v\n", err)
		os.Exit(1)
	}

	if mode == "mock" {
		runMockDemo(store)
	} else {
		debugPort, err := parseInt(mode)
		if err != nil {
			fmt.Fprintf(os.Stderr, "无效的调试端口: %s\n", mode)
			os.Exit(1)
		}
		runRealDemo(store, debugPort)
	}
}

func runRealDemo(store *behavior.FileRecordingStore, debugPort int) {
	fmt.Println("=== 小红书养号 实时录制演示 ===")
	fmt.Printf("连接浏览器调试端口: %d\n", debugPort)
	fmt.Println()

	// 1. Navigate to Xiaohongshu
	fmt.Println("[1/5] 导航到小红书...")
	navigateTo(debugPort, "https://www.xiaohongshu.com/explore")
	time.Sleep(3 * time.Second)

	// 2. Start recording + auto-nurture
	fmt.Println("[2/5] 开始录制 + 养号行为...")
	name := fmt.Sprintf("小红书养号 %s", time.Now().Format("2006-01-02 15:04"))
	recording, err := behavior.AutoRecord(debugPort, name, behavior.NurturingActions())
	if err != nil {
		fmt.Fprintf(os.Stderr, "自动录制失败: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("  ✓ 录制完成: %d 事件, %.1fs\n", len(recording.Events), float64(recording.DurationMs)/1000)

	// 3. Save recording
	fmt.Println("[3/5] 保存录制...")
	if err := store.Save(recording); err != nil {
		fmt.Fprintf(os.Stderr, "保存失败: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("  ✓ 已保存: %s\n", recording.ID)

	// 4. Generate offset variants
	fmt.Println("[4/5] 生成50方向偏移变体...")
	saveOffsetVariants(recording.ID)

	// 5. Print summary
	printSummary(recording)
}

func runMockDemo(store *behavior.FileRecordingStore) {
	fmt.Println("=== 小红书养号 模拟演示 (生成示例数据) ===")
	fmt.Println()

	// 1. Generate a realistic recording
	fmt.Println("[1/4] 生成模拟录制数据...")
	recording := generateMockXHSRecording()
	fmt.Printf("  ✓ 生成完成: %d 事件, %.1fs\n", len(recording.Events), float64(recording.DurationMs)/1000)

	// 2. Save
	fmt.Println("[2/4] 保存到录制存储...")
	if err := store.Save(recording); err != nil {
		fmt.Fprintf(os.Stderr, "保存失败: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("  ✓ 已保存: %s\n", recording.ID)

	// 3. Generate offset variants
	fmt.Println("[3/4] 生成50方向偏移变体...")
	saveOffsetVariants(recording.ID)

	// 4. Summary
	fmt.Println("[4/4] 生成各类偏移模板...")
	printSummary(recording)
}

func generateMockXHSRecording() *behavior.Recording {
	rng := rand.New(rand.NewSource(time.Now().UnixNano()))
	startTime := time.Now()
	events := make([]behavior.RecordedEvent, 0, 200)

	// Simulate: 打开小红书 → 浏览 → 点赞 → 滚动 → 搜索 → 浏览 → 等待
	type step struct {
		desc    string
		durMs   int64
		actions func(int64) []behavior.RecordedEvent
	}

	steps := []step{
		{"页面加载", 2500, func(baseT int64) []behavior.RecordedEvent {
			return []behavior.RecordedEvent{
				{T: baseT, Type: "move", X: 800, Y: 400},
				{T: baseT + 300, Type: "move", X: 960, Y: 540},
			}
		}},
		{"浏览首页内容", 4000, func(baseT int64) []behavior.RecordedEvent {
			evts := []behavior.RecordedEvent{}
			// Scroll down with realistic steps
			y := 540.0
			for i := int64(0); i < 8; i++ {
				y -= float64(100 + rng.Intn(200))
				evts = append(evts, behavior.RecordedEvent{
					T: baseT + i*400, Type: "scroll", X: 960, Y: y, DeltaY: float64(-120 - rng.Intn(80)),
				})
				if i%3 == 0 {
					// Pause to "read"
					evts = append(evts, behavior.RecordedEvent{
						T: baseT + i*400 + 200, Type: "move", X: float64(800 + rng.Intn(300)), Y: float64(300 + rng.Intn(400)),
					})
				}
			}
			return evts
		}},
		{"点赞操作", 3000, func(baseT int64) []behavior.RecordedEvent {
			evts := []behavior.RecordedEvent{}
			// Click like button
			lx, ly := 900.0, 350.0
			evts = append(evts, behavior.RecordedEvent{T: baseT, Type: "move", X: 850, Y: 400})
			evts = append(evts, behavior.RecordedEvent{T: baseT + 200, Type: "move", X: lx, Y: ly})
			evts = append(evts, behavior.RecordedEvent{T: baseT + 500, Type: "down", X: lx + 2, Y: ly + 1, Button: 0})
			evts = append(evts, behavior.RecordedEvent{T: baseT + 560, Type: "up", X: lx + 2, Y: ly + 1, Button: 0})
			evts = append(evts, behavior.RecordedEvent{T: baseT + 570, Type: "click", X: lx + 2, Y: ly + 1, Button: 0})
			return evts
		}},
		{"继续浏览", 5000, func(baseT int64) []behavior.RecordedEvent {
			evts := []behavior.RecordedEvent{}
			y := 300.0
			for i := int64(0); i < 12; i++ {
				y -= float64(80 + rng.Intn(150))
				evts = append(evts, behavior.RecordedEvent{
					T: baseT + i*350, Type: "scroll", X: 960, Y: y, DeltaY: float64(-100 - rng.Intn(60)),
				})
			}
			return evts
		}},
		{"点击笔记详情", 2000, func(baseT int64) []behavior.RecordedEvent {
			lx, ly := 500.0+float64(rng.Intn(300)), 400.0+float64(rng.Intn(300))
			return []behavior.RecordedEvent{
				{T: baseT, Type: "move", X: 960, Y: 500},
				{T: baseT + 300, Type: "move", X: lx, Y: ly},
				{T: baseT + 400, Type: "down", X: lx, Y: ly, Button: 0},
				{T: baseT + 460, Type: "up", X: lx, Y: ly, Button: 0},
				{T: baseT + 470, Type: "click", X: lx, Y: ly, Button: 0},
			}
		}},
		{"阅读笔记内容", 6000, func(baseT int64) []behavior.RecordedEvent {
			evts := []behavior.RecordedEvent{}
			y := 400.0
			for i := int64(0); i < 15; i++ {
				y -= float64(60 + rng.Intn(100))
				evts = append(evts, behavior.RecordedEvent{
					T: baseT + i*350, Type: "scroll", X: 960, Y: y, DeltaY: float64(-80 - rng.Intn(50)),
				})
				if i%5 == 0 {
					evts = append(evts, behavior.RecordedEvent{
						T: baseT + i*350 + 100, Type: "move", X: float64(700 + rng.Intn(400)), Y: float64(200 + rng.Intn(400)),
					})
				}
			}
			return evts
		}},
		{"点赞评论", 1500, func(baseT int64) []behavior.RecordedEvent {
			lx, ly := 880.0, 700.0
			return []behavior.RecordedEvent{
				{T: baseT, Type: "move", X: 900, Y: 650},
				{T: baseT + 200, Type: "move", X: lx, Y: ly},
				{T: baseT + 350, Type: "click", X: lx, Y: ly, Button: 0},
			}
		}},
		{"返回首页", 2000, func(baseT int64) []behavior.RecordedEvent {
			return []behavior.RecordedEvent{
				{T: baseT, Type: "move", X: 50, Y: 50},
				{T: baseT + 300, Type: "click", X: 45, Y: 45, Button: 0},
			}
		}},
		{"最后浏览", 3000, func(baseT int64) []behavior.RecordedEvent {
			evts := []behavior.RecordedEvent{}
			y := 540.0
			for i := int64(0); i < 6; i++ {
				y -= float64(100 + rng.Intn(150))
				evts = append(evts, behavior.RecordedEvent{
					T: baseT + i*400, Type: "scroll", X: 960, Y: y, DeltaY: float64(-100 - rng.Intn(60)),
				})
			}
			return evts
		}},
	}

	var cumulativeT int64
	for _, step := range steps {
		stepEvents := step.actions(cumulativeT)
		events = append(events, stepEvents...)
		if len(stepEvents) > 0 {
			cumulativeT = stepEvents[len(stepEvents)-1].T + step.durMs
		} else {
			cumulativeT += step.durMs
		}
	}

	return &behavior.Recording{
		ID:          fmt.Sprintf("rec-xhs-%d", time.Now().UnixMilli()),
		Name:        fmt.Sprintf("小红书养号 %s", startTime.Format("2006-01-02 15:04")),
		Description: "自动生成的小红书养号行为录制 (打开→浏览→点赞→评论→返回→继续浏览)",
		Events:      events,
		DurationMs:  cumulativeT,
		ViewportW:   1920,
		ViewportH:   1080,
		CreatedAt:   startTime.Format(time.RFC3339),
	}
}

func saveOffsetVariants(recordingID string) {
	lib := offsets.BuiltinOffsetLibrary()
	data, _ := json.MarshalIndent(map[string]interface{}{
		"recordingId": recordingID,
		"totalCount":  len(lib),
		"byCategory":  groupByCategory(lib),
		"variants":    lib,
	}, "", "  ")
	os.WriteFile("data/recordings/"+recordingID+"-offsets.json", data, 0644)
	fmt.Printf("  ✓ 偏移变体: %d 个 (open:%d browse:%d click:%d type:%d wait:%d)\n",
		len(lib),
		countByCategory(lib, "open"),
		countByCategory(lib, "browse"),
		countByCategory(lib, "click"),
		countByCategory(lib, "type"),
		countByCategory(lib, "wait"),
	)
}

func groupByCategory(lib []offsets.OffsetVariant) map[string]int {
	result := make(map[string]int)
	for _, v := range lib {
		result[v.Category]++
	}
	return result
}

func countByCategory(lib []offsets.OffsetVariant, cat string) int {
	n := 0
	for _, v := range lib {
		if v.Category == cat {
			n++
		}
	}
	return n
}

func navigateTo(debugPort int, url string) {
	ws, err := behavior.ConnectPageCDP(debugPort)
	if err != nil {
		return
	}
	defer ws.Close()
	ws.WriteJSON(map[string]interface{}{
		"id":     1,
		"method": "Page.navigate",
		"params": map[string]interface{}{"url": url},
	})
}

func printSummary(rec *behavior.Recording) {
	fmt.Println()
	fmt.Println("========================================")
	fmt.Println("  小红书养号录制 + 50方向偏移 完成!")
	fmt.Println("========================================")
	fmt.Printf("  录制ID:     %s\n", rec.ID)
	fmt.Printf("  录制名称:   %s\n", rec.Name)
	fmt.Printf("  事件总数:   %d\n", len(rec.Events))
	fmt.Printf("  录制时长:   %.1fs\n", float64(rec.DurationMs)/1000)
	fmt.Printf("  视口尺寸:   %dx%d\n", rec.ViewportW, rec.ViewportH)
	fmt.Println()
	fmt.Println("  偏移分类:")
	lib := offsets.BuiltinOffsetLibrary()
	for _, cat := range []string{"open", "browse", "click", "type", "wait"} {
		fmt.Printf("    %-10s: %2d 个方向\n", offsets.CategoryHumanNames[cat], countByCategory(lib, cat))
	}
	fmt.Println()
	fmt.Println("  数据文件:")
	fmt.Printf("    data/recordings/%s.json\n", rec.ID)
	fmt.Printf("    data/recordings/%s-offsets.json\n", rec.ID)
	fmt.Println()
	fmt.Println("  打开应用 → 行为录制页面 → 即可查看可视化结果")
	fmt.Println("========================================")
}

func parseInt(s string) (int, error) {
	n := 0
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("invalid integer: %s", s)
		}
		n = n*10 + int(c-'0')
	}
	return n, nil
}
