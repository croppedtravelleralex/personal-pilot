package detection

import (
	"encoding/json"
	"regexp"
	"strconv"
	"strings"
)

var (
	creepTrustRe      = regexp.MustCompile(`(?i)trust\s*score[:\s]+(\d+(?:\.\d+)?)`)
	creepLiesCountRe  = regexp.MustCompile(`(?i)lies?\s*(?:detected|count)?[:\s]+(\d+)`)
	creepLieLineRe    = regexp.MustCompile(`(?i)\blie\b`)
	creepGradeRe      = regexp.MustCompile(`(?i)\bgrade[:\s]+([A-F][+\-]?)`)
	creepFPIDRe       = regexp.MustCompile(`(?i)(?:fp\s*id|fingerprint\s*id)[:\s]+([a-z0-9\-_.]{6,})`)
	creepBotLikeRe    = regexp.MustCompile(`(?i)\b(bot|headless|automation|puppeteer|playwright|selenium)\b`)
)

// SiteProbeResult captures parsed third-party detector output.
type SiteProbeResult struct {
	SiteID           string   `json:"siteId"`
	TrustScore       float64  `json:"trustScore"`
	Grade            string   `json:"grade,omitempty"`
	FingerprintID    string   `json:"fingerprintId,omitempty"`
	Webdriver        bool     `json:"webdriver"`
	LiesDetected     int      `json:"liesDetected"`
	Lies             []string `json:"lies,omitempty"`
	HeadlessHints    []string `json:"headlessHints,omitempty"`
	WorkerConsistent *bool    `json:"workerConsistent,omitempty"`
	Source           string   `json:"source,omitempty"` // "structured" | "parsed" | "heuristic"
	RawSnippet       string   `json:"rawSnippet,omitempty"`
	ParseOK          bool     `json:"parseOk"`
	Message          string   `json:"message"`
}

// creepJSProbeJS scrapes CreepJS with structured DOM selectors first, body text fallback second.
const creepJSProbeJS = `(async function(){
  function textOf(el) {
    try { return (el && (el.innerText || el.textContent) || '').trim(); } catch (_) { return ''; }
  }
  function pickNumber(text) {
    if (!text) return null;
    const m = String(text).match(/(\d+(?:\.\d+)?)/);
    return m ? Number(m[1]) : null;
  }
  const body = document.body ? document.body.innerText : '';
  const html = document.documentElement ? document.documentElement.innerHTML : '';

  // Structured selectors used by abrahamjuliot/creepjs UI variants.
  const selectors = {
    trust: [
      '[class*="trust"]', '[data-trust]', '#trust-score', '.trust-score',
      'span', 'div', 'strong', 'b', 'h1', 'h2', 'h3'
    ],
    lies: [
      '[class*="lie"]', '[data-lies]', '#lies', '.lies', 'li', 'div', 'span'
    ],
    grade: [
      '[class*="grade"]', '#grade', '.grade'
    ],
    fpid: [
      '[class*="fp"]', '#fp-id', '.fp-id', 'code', 'span', 'div'
    ]
  };

  let trustScore = null;
  let grade = '';
  let fingerprintId = '';
  let lieLines = [];
  let liesCount = 0;
  let structured = false;

  // Trust score: prefer nodes whose text includes "trust"
  try {
    const all = Array.from(document.querySelectorAll('body *')).slice(0, 2500);
    for (const el of all) {
      const t = textOf(el);
      if (!t || t.length > 120) continue;
      if (/trust\s*score/i.test(t) && trustScore == null) {
        const n = pickNumber(t);
        if (n != null && n >= 0 && n <= 100) { trustScore = n; structured = true; }
      }
      if (/\bgrade\b/i.test(t) && !grade) {
        const gm = t.match(/\bgrade[:\s]+([A-F][+\-]?)/i);
        if (gm) { grade = gm[1].toUpperCase(); structured = true; }
      }
      if (/(fp\s*id|fingerprint\s*id)/i.test(t) && !fingerprintId) {
        const fm = t.match(/(?:fp\s*id|fingerprint\s*id)[:\s]+([a-z0-9\-_.]{6,})/i);
        if (fm) { fingerprintId = fm[1]; structured = true; }
      }
      if (/\blie\b/i.test(t) && t.length < 160) {
        lieLines.push(t);
      }
      if (/lies?\s*(detected|count)?[:\s]+\d+/i.test(t)) {
        const n = pickNumber(t);
        if (n != null) { liesCount = Math.max(liesCount, n); structured = true; }
      }
    }
  } catch (_) {}

  // Unique lie lines
  const seen = {};
  lieLines = lieLines.filter(function(l) {
    const k = l.toLowerCase();
    if (seen[k]) return false;
    seen[k] = true;
    return true;
  }).slice(0, 30);
  if (liesCount === 0) liesCount = lieLines.length;

  // Body regex fallbacks
  if (trustScore == null) {
    const m = body.match(/trust\s*score[:\s]+(\d+(?:\.\d+)?)/i);
    if (m) trustScore = Number(m[1]);
  }
  if (!grade) {
    const m = body.match(/\bgrade[:\s]+([A-F][+\-]?)/i);
    if (m) grade = m[1].toUpperCase();
  }
  if (!fingerprintId) {
    const m = body.match(/(?:fp\s*id|fingerprint\s*id)[:\s]+([a-z0-9\-_.]{6,})/i);
    if (m) fingerprintId = m[1];
  }
  if (liesCount === 0) {
    const m = body.match(/lies?\s*(?:detected|count)?[:\s]+(\d+)/i);
    if (m) liesCount = Number(m[1]);
  }
  if (lieLines.length === 0) {
    lieLines = body.split('\n').map(function(l){ return l.trim(); }).filter(function(l){ return /\blie\b/i.test(l); }).slice(0, 20);
    if (liesCount === 0) liesCount = lieLines.length;
  }

  const headlessHints = [];
  if (/headless/i.test(body) || /headless/i.test(html)) headlessHints.push('headless');
  if (/HeadlessChrome/i.test(body)) headlessHints.push('HeadlessChrome');
  if (/chrome-headless/i.test(body)) headlessHints.push('chrome-headless');
  if (/puppeteer|playwright|selenium/i.test(body)) headlessHints.push('automation-framework');
  const workerMismatch = /worker.*(mismatch|inconsist|lie)/i.test(body);

  // Computing state: page still rendering
  const computing = /computing|loading|please wait/i.test(body) && trustScore == null;

  return {
    webdriver: !!navigator.webdriver,
    userAgent: navigator.userAgent || '',
    bodySnippet: body.slice(0, 6000),
    trustScore: trustScore,
    grade: grade,
    fingerprintId: fingerprintId,
    lies: liesCount,
    lieLines: lieLines,
    headlessHints: headlessHints,
    workerMismatch: workerMismatch,
    structured: structured && trustScore != null,
    computing: computing
  };
})()`

// CreepJSProbeJS returns the in-page probe expression for CDP evaluate.
func CreepJSProbeJS() string { return creepJSProbeJS }

// ParseCreepJSProbePayload interprets CDP evaluate output from a CreepJS page.
// Source priority: structured (DOM) > parsed (regex on body) > heuristic.
func ParseCreepJSProbePayload(raw []byte) SiteProbeResult {
	out := SiteProbeResult{SiteID: "creepjs"}
	var payload struct {
		Webdriver      bool     `json:"webdriver"`
		BodySnippet    string   `json:"bodySnippet"`
		TrustScore     *float64 `json:"trustScore"`
		Grade          string   `json:"grade"`
		FingerprintID  string   `json:"fingerprintId"`
		Lies           int      `json:"lies"`
		LieLines       []string `json:"lieLines"`
		HeadlessHints  []string `json:"headlessHints"`
		WorkerMismatch bool     `json:"workerMismatch"`
		Structured     bool     `json:"structured"`
		Computing      bool     `json:"computing"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		out.Message = "parse failed"
		return out
	}
	out.Webdriver = payload.Webdriver
	out.LiesDetected = payload.Lies
	out.RawSnippet = payload.BodySnippet
	out.Grade = strings.TrimSpace(payload.Grade)
	out.FingerprintID = strings.TrimSpace(payload.FingerprintID)
	out.Lies = append([]string(nil), payload.LieLines...)
	out.HeadlessHints = append([]string(nil), payload.HeadlessHints...)
	workerConsistent := !payload.WorkerMismatch
	out.WorkerConsistent = &workerConsistent

	// Enrich lies from body if needed.
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

	// 1) Structured DOM trust score
	if payload.TrustScore != nil && *payload.TrustScore >= 0 && *payload.TrustScore <= 100 {
		out.TrustScore = *payload.TrustScore
		out.ParseOK = true
		if payload.Structured {
			out.Source = "structured"
			out.Message = "structured DOM trust score"
		} else {
			out.Source = "parsed"
			out.Message = "trust score extracted from page payload"
		}
		// Prefer explicit lies count from body if higher fidelity.
		if m := creepLiesCountRe.FindStringSubmatch(payload.BodySnippet); len(m) == 2 {
			if n, err := strconv.Atoi(m[1]); err == nil && n > out.LiesDetected {
				out.LiesDetected = n
			}
		}
		return out
	}

	// 2) Regex parse from body snippet
	if m := creepTrustRe.FindStringSubmatch(payload.BodySnippet); len(m) == 2 {
		if v, err := strconv.ParseFloat(m[1], 64); err == nil {
			out.TrustScore = v
			out.ParseOK = true
			out.Source = "parsed"
			out.Message = "trust score extracted"
			if out.Grade == "" {
				if gm := creepGradeRe.FindStringSubmatch(payload.BodySnippet); len(gm) == 2 {
					out.Grade = strings.ToUpper(gm[1])
				}
			}
			if out.FingerprintID == "" {
				if fm := creepFPIDRe.FindStringSubmatch(payload.BodySnippet); len(fm) == 2 {
					out.FingerprintID = fm[1]
				}
			}
			return out
		}
	}

	// 3) Heuristic fallback when page still computing or trust text missing.
	score := 100.0
	if payload.Webdriver {
		score -= 40
	}
	if payload.Lies > 0 {
		score -= float64(payload.Lies) * 3
	} else if len(out.Lies) > 0 {
		score -= float64(len(out.Lies)) * 3
	}
	for _, h := range out.HeadlessHints {
		if creepBotLikeRe.MatchString(h) {
			score -= 8
		}
	}
	if score < 0 {
		score = 0
	}
	out.TrustScore = score
	out.ParseOK = true
	out.Source = "heuristic"
	if payload.Computing {
		out.Message = "heuristic trust; page still computing"
	} else {
		out.Message = "heuristic trust from webdriver/lies"
	}
	return out
}

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
