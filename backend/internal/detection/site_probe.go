package detection

import (
	"encoding/json"
	"regexp"
	"strconv"
	"strings"
)

var creepTrustRe = regexp.MustCompile(`(?i)trust\s*score[:\s]+(\d+(?:\.\d+)?)`)

// SiteProbeResult captures parsed third-party detector output.
type SiteProbeResult struct {
	SiteID           string   `json:"siteId"`
	TrustScore       float64  `json:"trustScore"`
	Webdriver        bool     `json:"webdriver"`
	LiesDetected     int      `json:"liesDetected"`
	Lies             []string `json:"lies,omitempty"`
	HeadlessHints    []string `json:"headlessHints,omitempty"`
	WorkerConsistent *bool    `json:"workerConsistent,omitempty"`
	Source           string   `json:"source,omitempty"` // "parsed" | "heuristic"
	RawSnippet       string   `json:"rawSnippet,omitempty"`
	ParseOK          bool     `json:"parseOk"`
	Message          string   `json:"message"`
}

const creepJSProbeJS = `(function(){
  const body = document.body ? document.body.innerText : '';
  const lies = (body.match(/lie/gi) || []).length;
  const lieLines = body.split('\n').map(function(l){ return l.trim(); }).filter(function(l){ return /lie/i.test(l); }).slice(0, 20);
  const headlessHints = [];
  if (/headless/i.test(body)) headlessHints.push('headless');
  if (/HeadlessChrome/i.test(body)) headlessHints.push('HeadlessChrome');
  if (/chrome-headless/i.test(body)) headlessHints.push('chrome-headless');
  const workerMismatch = /worker.*(mismatch|inconsist|lie)/i.test(body);
  return {
    webdriver: !!navigator.webdriver,
    userAgent: navigator.userAgent || '',
    bodySnippet: body.slice(0, 4000),
    lies: lies,
    lieLines: lieLines,
    headlessHints: headlessHints,
    workerMismatch: workerMismatch
  };
})()`

// CreepJSProbeJS returns the in-page probe expression for CDP evaluate.
func CreepJSProbeJS() string { return creepJSProbeJS }

// ParseCreepJSProbePayload interprets CDP evaluate output from a CreepJS page.
func ParseCreepJSProbePayload(raw []byte) SiteProbeResult {
	out := SiteProbeResult{SiteID: "creepjs"}
	var payload struct {
		Webdriver     bool     `json:"webdriver"`
		BodySnippet   string   `json:"bodySnippet"`
		Lies          int      `json:"lies"`
		LieLines      []string `json:"lieLines"`
		HeadlessHints []string `json:"headlessHints"`
		WorkerMismatch bool    `json:"workerMismatch"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		out.Message = "parse failed"
		return out
	}
	out.Webdriver = payload.Webdriver
	out.LiesDetected = payload.Lies
	out.RawSnippet = payload.BodySnippet
	out.Lies = append([]string(nil), payload.LieLines...)
	if len(out.Lies) == 0 && payload.BodySnippet != "" {
		for _, line := range strings.Split(payload.BodySnippet, "\n") {
			line = strings.TrimSpace(line)
			if line != "" && creepLieLineRe.MatchString(line) {
				out.Lies = append(out.Lies, line)
				if len(out.Lies) >= 20 {
					break
				}
			}
		}
	}
	if len(out.Lies) > 0 && out.LiesDetected == 0 {
		out.LiesDetected = len(out.Lies)
	}
	out.HeadlessHints = append([]string(nil), payload.HeadlessHints...)
	workerConsistent := !payload.WorkerMismatch
	out.WorkerConsistent = &workerConsistent

	if m := creepTrustRe.FindStringSubmatch(payload.BodySnippet); len(m) == 2 {
		if v, err := strconv.ParseFloat(m[1], 64); err == nil {
			out.TrustScore = v
			out.ParseOK = true
			out.Source = "parsed"
			out.Message = "trust score extracted"
			return out
		}
	}
	// Heuristic when trust score text not found
	score := 100.0
	if payload.Webdriver {
		score -= 40
	}
	if payload.Lies > 0 {
		score -= float64(payload.Lies) * 3
	}
	if len(out.Lies) > 0 && payload.Lies == 0 {
		score -= float64(len(out.Lies)) * 3
	}
	if score < 0 {
		score = 0
	}
	out.TrustScore = score
	out.ParseOK = true
	out.Source = "heuristic"
	out.Message = "heuristic trust from webdriver/lies"
	return out
}

var creepLieLineRe = regexp.MustCompile(`(?i)lie`)

// ParseBrowserleaksIPBody checks exit IP visibility consistency hints.
func ParseBrowserleaksIPBody(body string, expectedExitIP string) SiteProbeResult {
	out := SiteProbeResult{SiteID: "browserleaks_ip", ParseOK: true}
	body = strings.ToLower(body)
	expected := strings.TrimSpace(strings.ToLower(expectedExitIP))
	if expected != "" && strings.Contains(body, expected) {
		out.TrustScore = 95
		out.Message = "exit IP visible on browserleaks"
	} else if expected != "" {
		out.TrustScore = 40
		out.Message = "exit IP mismatch on browserleaks"
	} else {
		out.TrustScore = 70
		out.Message = "browserleaks loaded; exit IP unknown"
	}
	return out
}
