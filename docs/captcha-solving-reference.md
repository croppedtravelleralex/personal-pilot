# CAPTCHA Solving Services — Comprehensive Reference (May 2026)

> **Purpose:** Reference document for integrating CAPTCHA solving into a browser automation platform (similar to AdsPower/Multilogin). Covers service APIs, pricing, SDKs, open-source OCR, and architecture.

---

## Table of Contents

1. [Service Comparison Matrix](#1-service-comparison-matrix)
2. [2Captcha API](#2-2captcha-api)
3. [Capsolver API](#3-capsolver-api)
4. [Anti-Captcha API](#4-anti-captcha-api)
5. [ddddocr (Open-Source)](#5-ddddocr-open-source)
6. [PaddleOCR](#6-paddleocr)
7. [BrightData CAPTCHA Solving](#7-brightdata-captcha-solving)
8. [Go & Rust SDK Availability](#8-go--rust-sdk-availability)
9. [Integration Architecture](#9-integration-architecture)
10. [Retry Strategies & Cost Optimization](#10-retry-strategies--cost-optimization)

---

## 1. Service Comparison Matrix

### Pricing Per 1,000 Solves (USD, 2026)

| CAPTCHA Type | 2Captcha | CapSolver | Anti-Captcha | CapMonster |
|---|---|---|---|---|
| reCAPTCHA v2 | $1.00 - $2.99 | $1.00 - $2.50 | $1.00 - $2.00 | $0.60 - $1.20 |
| reCAPTCHA v3 | $1.45 - $2.99 | $1.50 - $3.00 | $1.40 - $2.50 | $1.30 |
| Cloudflare Turnstile | $1.00 - $2.99 | $0.60 - $1.50 | $1.00 - $2.00 | $0.50 |
| hCaptcha | $1.00 - $2.99 | $0.80 - $1.50 | $0.95 - $2.00 | $0.60 |
| FunCaptcha (Arkose) | $1.00 - $2.99 | $1.50 | $2.00 | $1.80 |
| GeeTest v3 | $2.99 | $1.20 | $2.00 | $1.50 |
| GeeTest v4 | $2.99 | $1.50 | N/A | $1.80 |
| Image-to-Text | $0.50 - $1.00 | $0.20 - $0.50 | $0.50 - $0.70 | $0.30 |

### Speed Comparison

| Provider | Model | Avg Speed (Token CAPTCHAs) |
|---|---|---|
| 2Captcha | Hybrid (AI + Human) | 15 - 45s |
| CapSolver | AI Only | 3 - 8s |
| Anti-Captcha | Hybrid (AI + Human) | 12 - 35s |
| CapMonster Cloud | AI Only | 3 - 10s |

### Provider Strengths

| Provider | Best For | Weakness |
|---|---|---|
| **2Captcha** | Broadest type coverage, highest reliability | Slowest avg speed, dynamic pricing |
| **CapSolver** | Speed, modern CAPTCHAs (Turnstile), cheap image | Smaller workforce for edge cases |
| **Anti-Captcha** | Best Go SDK, no concurrency limits | Slightly pricier for complex types |
| **CapMonster** | Cheapest overall, fast AI-only | Limited complex CAPTCHA support |

---

## 2. 2Captcha API

### API Endpoints

| Endpoint | URL | Purpose |
|---|---|---|
| Upload/Submit | `https://2captcha.com/in.php` | Submit CAPTCHA, get task ID |
| Retrieve Result | `https://2captcha.com/res.php` | Poll for solved result |
| Balance | `https://2captcha.com/res.php?action=getbalance` | Check account balance |
| Report Bad | `https://2captcha.com/res.php?action=reportbad` | Report incorrect solve |

### Flow: in.php → res.php

```
1. POST to in.php with CAPTCHA data → get task ID ("OK|2122988149")
2. Wait 5+ seconds
3. GET from res.php with task ID → get result ("OK|TEXT") or "CAPCHA_NOT_READY"
4. Repeat step 3 every 5s until solved or timeout
```

### Normal CAPTCHA (Image-to-Text)

```bash
# Submit (file upload)
curl -F "method=post" \
     -F "key=YOUR_API_KEY" \
     -F "file=@captcha.jpg" \
     https://2captcha.com/in.php
# → OK|2122988149

# Submit (base64)
curl -X POST https://2captcha.com/in.php \
  -d "method=base64" \
  -d "key=YOUR_API_KEY" \
  -d "body=BASE64_ENCODED_IMAGE"

# Retrieve
curl "https://2captcha.com/res.php?key=YOUR_API_KEY&action=get&id=2122988149"
# → OK|w9g7a  (solved text)  OR  CAPCHA_NOT_READY
```

### reCAPTCHA v2 (NoCaptchaTaskProxyless)

```bash
# Submit
curl -X POST https://2captcha.com/in.php \
  -d "key=YOUR_API_KEY" \
  -d "method=userrecaptcha" \
  -d "googlekey=6Lc..._xxx" \
  -d "pageurl=https://example.com" \
  -d "json=1"

# Response: {"status":1,"request":"2122988149"}

# Retrieve
curl "https://2captcha.com/res.php?key=YOUR_API_KEY&action=get&id=2122988149&json=1"
# → {"status":1,"request":"03AGdBq25..."}  (g-recaptcha-response token)
```

### Key Task Types

| Task Type | `method` / `type` parameter | Description |
|---|---|---|
| Image CAPTCHA | `post` or `base64` | Standard image text |
| reCAPTCHA v2 | `userrecaptcha` | reCAPTCHA v2 checkbox/invisible |
| reCAPTCHA v3 | `userrecaptcha` (with `version=v3`) | reCAPTCHA v3 score-based |
| reCAPTCHA Enterprise | `userrecaptcha` (with `enterprise=1`) | Enterprise variant |
| GeeTest | `geetest` | GeeTest v3 slider/puzzle |
| GeeTest v4 | `geetest_v4` | GeeTest v4 |
| FunCaptcha (Arkose) | `funcaptcha` | Arkose Labs |
| Cloudflare Turnstile | `turnstile` | Turnstile token |
| KeyCaptcha | `keycaptcha` | KeyCAPTCHA |
| Capy Puzzle | `capy` | Capy CAPTCHA |
| Amazon WAF | `amazon_waf` | AWS WAF CAPTCHA |
| Audio | `audio` | Audio CAPTCHA (MP3/WAV) |
| Text | `text` | Free-form text question |
| Coordinates | `coordinates` | Click-on-image |
| Rotate | `rotate` | Rotate image |
| Grid | `grid` | 3x3 grid selection |

### Additional Parameters

| Parameter | Type | Description |
|---|---|---|
| `key` | String | API key (always required) |
| `method` | String | CAPTCHA type |
| `json` | Int | Return JSON (1) instead of plain text |
| `soft_id` | Int | Software ID for affiliate tracking |
| `pingback` | URL | Callback URL for async result delivery |
| `language` | Int | 0=not set, 1=Russian, 2=English |
| `header_acao` | Int | Add CORS header (1) |
| `regsense` | Int | Case-sensitive (1) |
| `numeric` | Int | 0=any, 1=only numbers, 2=only letters |
| `min_len` / `max_len` | Int | Answer length range |
| `phrase` | Int | 0=one word, 1=two words |
| `enterprise` | Int | reCAPTCHA Enterprise flag |

### Pingback (Async) Mode

Submit with `&pingback=https://your-server.com/callback` → 2Captcha POSTs result to your URL when ready. No polling needed.

### Go SDK

```go
import "github.com/2captcha/2captcha-go"

client := api2captcha.NewClient("YOUR_API_KEY")

// Image captcha
cap := api2captcha.Normal{CaptchaBase64: "base64..."}
code, err := client.Solve(cap)

// reCAPTCHA v2
cap := api2captcha.ReCaptcha{
  SiteKey: "6Lc..._xxx",
  Url:     "https://example.com",
}
code, err := client.Solve(cap)

// FunCaptcha
cap := api2captcha.FunCaptcha{
  PublicKey: "ABC...",
  PageUrl:   "https://example.com",
  SUrl:      "https://api.arkoselabs.com",
}
code, err := client.Solve(cap)

// GeeTest
cap := api2captcha.GeeTest{
  GT:        "f2bf4445e1c8d1f3...",
  Challenge: "9566103355b8c79f...",
  Url:       "https://example.com",
}
code, err := client.Solve(cap)

// Turnstile
cap := api2captcha.Turnstile{
  SiteKey: "0x4AAAAAAA...",
  Url:     "https://example.com",
}
code, err := client.Solve(cap)
```

### Rust SDK

No official 2Captcha Rust SDK. Community options:
- `https://crates.io/crates/2captcha` (community, basic)
- Or implement raw HTTP client against `in.php`/`res.php`

---

## 3. CapSolver API

### API Endpoints

| Endpoint | URL |
|---|---|
| Create Task | `POST https://api.capsolver.com/createTask` |
| Get Task Result | `POST https://api.capsolver.com/getTaskResult` |
| Get Balance | `POST https://api.capsolver.com/getBalance` |
| Feedback | `POST https://api.capsolver.com/feedbackTask` |

### Flow: createTask → getTaskResult

```
1. POST to /createTask → get taskId
2. POST to /getTaskResult with taskId → get status="ready" with solution
   or status="processing" (retry after ~1-3s)
```

### Image-to-Text (Synchronous)

```bash
curl -X POST https://api.capsolver.com/createTask \
  -H "Content-Type: application/json" \
  -d '{
    "clientKey": "CAI-xxx...",
    "task": {
      "type": "ImageToTextTask",
      "body": "BASE64_IMAGE_DATA"
    }
  }'

# Response (immediate):
{
  "errorId": 0,
  "status": "ready",
  "solution": { "text": "44795sds" },
  "taskId": "2376919c-..."
}
```

### reCAPTCHA v2 (Asynchronous)

```bash
curl -X POST https://api.capsolver.com/createTask \
  -H "Content-Type: application/json" \
  -d '{
    "clientKey": "CAI-xxx...",
    "task": {
      "type": "ReCaptchaV2TaskProxyLess",
      "websiteURL": "https://example.com",
      "websiteKey": "6Lc..._xxx"
    }
  }'

# Response:
{
  "errorId": 0,
  "taskId": "37223a89-..."
}

# Then poll:
curl -X POST https://api.capsolver.com/getTaskResult \
  -H "Content-Type: application/json" \
  -d '{
    "clientKey": "CAI-xxx...",
    "taskId": "37223a89-..."
  }'

# Response when ready:
{
  "errorId": 0,
  "status": "ready",
  "solution": {
    "gRecaptchaResponse": "03AGdBq25..."
  }
}
```

### Task Types

| Task Type | Description |
|---|---|
| `ImageToTextTask` | Simple image CAPTCHA (synchronous) |
| `ReCaptchaV2TaskProxyLess` | reCAPTCHA v2 (no proxy needed) |
| `ReCaptchaV2Task` | reCAPTCHA v2 (with proxy) |
| `ReCaptchaV2EnterpriseTask` | reCAPTCHA v2 Enterprise |
| `ReCaptchaV3TaskProxyLess` | reCAPTCHA v3 (no proxy) |
| `ReCaptchaV3Task` | reCAPTCHA v3 (with proxy) |
| `HCaptchaTaskProxyless` | hCaptcha |
| `HCaptchaTask` | hCaptcha (with proxy) |
| `FunCaptchaTask` | FunCaptcha / Arkose |
| `FunCaptchaTaskProxyLess` | FunCaptcha (no proxy) |
| `GeeTestTask` | GeeTest v3 |
| `GeeTestTaskProxyLess` | GeeTest v3 (no proxy) |
| `TurnstileTask` | Cloudflare Turnstile |
| `TurnstileTaskProxyless` | Cloudflare Turnstile (no proxy) |
| `AmazonWafTask` | AWS WAF CAPTCHA |
| `MTCaptchaTask` | MTCaptcha |
| `DataDomeTask` | DataDome |
| `FriendlyCaptchaTask` | Friendly Captcha |

### Go SDK

```go
import "github.com/capsolver/capsolver-go"

capSolver := CapSolver{ApiKey: "CAI-xxx..."}

// Solve reCAPTCHA v2
s, err := capSolver.Solve(map[string]any{
  "type":       "ReCaptchaV2TaskProxyLess",
  "websiteURL": "https://example.com",
  "websiteKey": "6Lc..._xxx",
})

// Solve image
b, _ := os.ReadFile("captcha.jpg")
s, err := capSolver.Solve(map[string]any{
  "type": "ImageToTextTask",
  "body": base64.StdEncoding.EncodeToString(b),
})

// Get balance
b, err := capSolver.Balance()
```

### Rust SDK

No official CapSolver Rust SDK. Raw HTTP implementation is straightforward (two POST endpoints).

---

## 4. Anti-Captcha API

### API Endpoints

| Endpoint | URL |
|---|---|
| Create Task | `POST https://api.anti-captcha.com/createTask` |
| Get Task Result | `POST https://api.anti-captcha.com/getTaskResult` |
| Get Balance | `POST https://api.anti-captcha.com/getBalance` |
| Get Queue Stats | `POST https://api.anti-captcha.com/getQueueStats` |
| Report Incorrect | `POST https://api.anti-captcha.com/reportIncorrectImage` |

### Flow

Same createTask → getTaskResult pattern as CapSolver.

### Example: Image-to-Text

```bash
curl -X POST https://api.anti-captcha.com/createTask \
  -H "Content-Type: application/json" \
  -d '{
    "clientKey": "YOUR_API_KEY",
    "task": {
      "type": "ImageToTextTask",
      "body": "BASE64_IMAGE",
      "phrase": false,
      "case": false,
      "numeric": 0,
      "math": false,
      "minLength": 0,
      "maxLength": 0
    }
  }'

# Response: {"errorId":0,"taskId":123456}

curl -X POST https://api.anti-captcha.com/getTaskResult \
  -H "Content-Type: application/json" \
  -d '{"clientKey":"YOUR_API_KEY","taskId":123456}'

# When solved: {"errorId":0,"status":"ready","solution":{"text":"w9g7a"}}
# While pending: {"errorId":0,"status":"processing"}
```

### Example: NoCaptchaTaskProxyless (reCAPTCHA v2)

```bash
curl -X POST https://api.anti-captcha.com/createTask \
  -H "Content-Type: application/json" \
  -d '{
    "clientKey": "YOUR_API_KEY",
    "task": {
      "type": "NoCaptchaTaskProxyless",
      "websiteURL": "https://example.com",
      "websiteKey": "6Lc..._xxx",
      "isInvisible": false
    }
  }'
```

### Task Types

| Task Type | Description |
|---|---|
| `ImageToTextTask` | Image CAPTCHA |
| `NoCaptchaTaskProxyless` | reCAPTCHA v2 (no proxy) |
| `NoCaptchaTask` | reCAPTCHA v2 (with proxy) |
| `RecaptchaV3TaskProxyless` | reCAPTCHA v3 |
| `RecaptchaV2EnterpriseTask` | reCAPTCHA v2 Enterprise |
| `RecaptchaV3EnterpriseTask` | reCAPTCHA v3 Enterprise |
| `HCaptchaTaskProxyless` | hCaptcha |
| `HCaptchaTask` | hCaptcha (with proxy) |
| `FunCaptchaTask` | FunCaptcha / Arkose |
| `GeeTestTask` | GeeTest |
| `TurnstileTaskProxyless` | Cloudflare Turnstile |
| `AntiGateTask` | Custom task (workers see webpage) |
| `ImageToCoordinatesTask` | Click-on-image |
| `FriendlyCaptchaTask` | Friendly Captcha |
| `AmazonWAF` | AWS WAF |
| `ProsopoTask` | Prosopo |

### Go SDK

```go
import "github.com/anti-captcha/anticaptcha-go"

ac := anticaptcha.NewClient("API_KEY_HERE")
ac.IsVerbose = true

balance, _ := ac.GetBalance()

// Image captcha
solution, _ := ac.SolveImageFile("captcha.jpg", anticaptcha.ImageSettings{})

// reCAPTCHA v2
solution, _ := ac.SolveRecaptchaV2(anticaptcha.RecaptchaV2Settings{
  WebsiteURL: "https://example.com",
  WebsiteKey: "6Lc..._xxx",
})

// Turnstile
solution, _ := ac.SolveTurnstile(anticaptcha.TurnstileSettings{
  WebsiteURL: "https://example.com",
  WebsiteKey: "0x4AAAAAAA...",
})

// GeeTest
solution, _ := ac.SolveGeeTest(anticaptcha.GeeTestSettings{
  GT:        "gt_hash",
  Challenge: "challenge_value",
  URL:       "https://example.com",
})

// hCaptcha
solution, _ := ac.SolveHCaptcha(anticaptcha.HCaptchaSettings{
  WebsiteURL: "https://example.com",
  WebsiteKey: "0000-xxxx",
})
```

### Rust SDK

No official Anti-Captcha Rust SDK. Use raw HTTP.

---

## 5. ddddocr (Open-Source)

### Overview

- **Author:** sml2h3 (Python), 86maid (Rust port)
- **License:** MIT
- **GitHub Python:** https://github.com/sml2h3/ddddocr (14.1k stars)
- **GitHub Rust:** https://github.com/86maid/ddddocr (316 stars)
- **Latest Python:** v1.5.6 (Mar 2026)
- **Latest Rust:** v6.0.6 (Apr 2026)

### How It Works

- **OCR Engine:** ONNX Runtime with pre-trained models
- **Detection:** Built-in text detection model (`det=True`)
- **Recognition:** CNN-based OCR trained on synthetic CAPTCHA data
- **Slider Matching:** OpenCV-based algorithm for slider CAPTCHA gap detection
- **No Preprocessing:** Designed to work without image pre-processing

### CAPTCHA Types Supported

| Type | Support | Notes |
|---|---|---|
| Standard alphanumeric text CAPTCHA | ✅ Good | Common English/numbers |
| Chinese text CAPTCHA | ✅ Partial | Some support |
| Slider/gap CAPTCHA | ✅ Good | Uses `slide_match()` |
| Object detection (click) | ✅ | Returns bounding boxes |
| reCAPTCHA, hCaptcha, Turnstile | ❌ | Not supported |

### Python Usage

```python
import ddddocr

# Basic OCR
ocr = ddddocr.DdddOcr()
with open('captcha.jpg', 'rb') as f:
    image = f.read()
result = ocr.classification(image)
print(result)  # "w9g7a"

# With probability
result = ocr.classification(image, probability=True)
# Returns probability distribution

# Restrict character set
ocr.set_ranges(0)                    # digits only
ocr.set_ranges("0123456789ABCDEF")   # custom charset

# Slider CAPTCHA
slide = ddddocr.DdddOcr(det=False, ocr=False)
with open('target.png', 'rb') as f: target = f.read()
with open('background.png', 'rb') as f: bg = f.read()
res = slide.slide_match(target, bg)
print(res)  # slider x-position

# Difference comparison
res = slide.slide_comparison(img_with_gap, img_without_gap)

# Object detection
det = ddddocr.DdddOcr(det=True, ocr=False)
poses = det.detection(image)
# Returns bounding boxes [[x1,y1,x2,y2], ...]

# Custom model
ocr = ddddocr.DdddOcr(
    det=False, ocr=False,
    import_onnx_path="my_model.onnx",
    charsets_path="charsets.json"
)
```

### Rust Usage (86maid/ddddocr)

```rust
use ddddocr::Ddddocr;

let mut ocr = Ddddocr::new("ddddocr.onnx")?;
let image_bytes = std::fs::read("captcha.png")?;
let result = ocr.classification(&image_bytes).await?;
println!("Recognized: {}", result);
```

### Deployment Options

| Method | Command | Notes |
|---|---|---|
| **Python CLI** | `python -m ddddocr` | Built-in HTTP server |
| **Docker** | `docker build -t ddddocr .` | Package with ONNX model |
| **Rust binary** | Pre-built binaries (Windows/Linux/macOS) | No OpenCV dependency, cross-platform |
| **MCP Server** | Built into Rust version | Tool-calling interface |
| **FastAPI wrapper** | Community wrappers available | REST API endpoints |

### Limitations

- No reCAPTCHA/hCaptcha/Turnstile token solving (only image OCR)
- Accuracy varies wildly — "depends on luck" per author
- Requires GPU for high throughput (>10 req/s)
- ONNX model files are large (~72MB)
- No native Chinese CAPTCHA training in default model

---

## 6. PaddleOCR

### Overview

- **Author:** Baidu (PaddlePaddle team)
- **License:** Apache 2.0
- **GitHub:** https://github.com/PaddlePaddle/PaddleOCR (75k stars)
- **Latest:** PaddleOCR-VL-1.5 (VLM-based, 2026)
- **Language:** Python, with Docker deployment

### CAPTCHA Relevance

PaddleOCR is **not a human verification solver** — it's a general-purpose document OCR engine. Use for:

- Chinese text CAPTCHA recognition (where ddddocr fails)
- Distorted/rotated text CAPTCHA (PaddleOCR handles orientation)
- Complex layout CAPTCHA (text + image combinations)

### Deployment

```bash
# Docker GPU deployment (production-ready)
docker run --gpus all --name paddleocr-api \
  -p 8000:8000 \
  paddleocr-api:latest

# Or Docker Compose (PaddleOCR-VL)
git clone https://github.com/PaddlePaddle/PaddleOCR.git
cd PaddleOCR/deploy/paddleocr_vl_docker
docker compose up -d

# Python package
pip install paddlepaddle-gpu==3.2.0
pip install "paddleocr[all]"

# Usage
from paddleocr import PaddleOCR
ocr = PaddleOCR(lang='ch')
result = ocr.ocr('captcha.jpg')
```

### Docker Compose Architecture (PaddleOCR-VL)

```
services:
  paddleocr-vlm-server:    # VLM inference (vLLM or FastDeploy)
    image: paddleocr-genai-vllm-server
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              capabilities: [gpu]

  paddleocr-vl-api:        # Pipeline API on port 8080
    image: paddleocr-vl-api
    ports: ["8080:8080"]
    depends_on: [paddleocr-vlm-server]
```

### Comparison with ddddocr for CAPTCHA

| Feature | ddddocr | PaddleOCR |
|---|---|---|
| CAPTCHA-focused | ✅ Yes | ❌ General OCR |
| Chinese CAPTCHA | ⚠️ Limited | ✅ Excellent |
| Speed | Fast (lightweight) | Slower (larger model) |
| GPU required | No (CPU OK) | Recommended |
| Deployment complexity | Low | Medium |
| Custom training | ✅ dddd_trainer | ✅ Full Paddle pipeline |

---

## 7. BrightData CAPTCHA Solving

### Overview

Bright Data does **not** offer a standalone CAPTCHA solving API. Instead, CAPTCHA solving is bundled into their **Scraping Browser** product:

- **Product:** Bright Data Browser API (Scraping Browser)
- **Mechanism:** Managed cloud browsers (Chromium) with built-in unblocking
- **CAPTCHA solving:** Automated as part of the browser session
- **Integration:** Puppeteer/Playwright connect via WebSocket

### How It Works

```javascript
const browser = await puppeteer.connect({
  browserWSEndpoint: `wss://connect.brightdata.com?token=API_KEY`
});
const page = await browser.newPage();
// Bright Data automatically:
// 1. Rotates residential proxies
// 2. Manages browser fingerprint
// 3. Detects and solves CAPTCHAs
// 4. Retries on failure
```

### Limitations

- **No standalone API** — you cannot solve a CAPTCHA independently (e.g., extract siteKey → get token)
- **Browser-only** — must route traffic through their managed browser
- **Expensive for CAPTCHA-only use** — pricing is per GB of bandwidth + proxy
- **Best for:** Full-site scraping workflows, not CAPTCHA-as-a-service

---

## 8. Go & Rust SDK Availability

### Go SDKs

| Service | Repository | Stars | Official | Status |
|---|---|---|---|---|
| **2Captcha** | `github.com/2captcha/2captcha-go` | 139 | ✅ Yes | Active (v1.1.10) |
| **CapSolver** | `github.com/capsolver/capsolver-go` | ~15 | ✅ Yes | Active (beta) |
| **Anti-Captcha** | `github.com/anti-captcha/anticaptcha-go` | 3 | ✅ Yes | Active (v1.0.11) |
| **ddddocr** | — (Python/Rust only) | — | ❌ No | N/A |

#### Go SDK Coverage Comparison

| Feature | 2Captcha | CapSolver | Anti-Captcha |
|---|---|---|---|
| ImageToText | ✅ | ✅ | ✅ |
| reCAPTCHA v2 | ✅ | ✅ | ✅ |
| reCAPTCHA v3 | ✅ | ✅ | ✅ |
| Enterprise | ✅ | ✅ | ✅ |
| FunCaptcha | ✅ | ✅ | ✅ |
| GeeTest v3/v4 | ✅ | ✅ | ✅ |
| Turnstile | ✅ | ✅ | ✅ |
| hCaptcha | ✅ | ✅ | ✅ |
| Amazon WAF | ✅ | ❌ | ✅ |
| FriendlyCaptcha | ✅ | ❌ | ✅ |
| Prosopo | ✅ | ❌ | ✅ |
| Async/polling | built-in | built-in | built-in |
| Balance check | ✅ | ✅ | ✅ |
| Report incorrect | ✅ | ✅ | ✅ |

### Rust SDKs

| Service | Repository | Official | Status |
|---|---|---|---|
| **2Captcha** | `crates.io/crates/2captcha` | ❌ Community | Minimal |
| **CapSolver** | — | ❌ | Raw HTTP only |
| **Anti-Captcha** | — | ❌ | Raw HTTP only |
| **ddddocr** | `github.com/86maid/ddddocr` | ✅ Active fork | v6.0.6, MCP support |
| **ddddocr-rs** | `github.com/mzdk100/ddddocr-rs` | ❌ Community | Basic |

---

## 9. Integration Architecture

### Typical Architecture for Browser Automation Platform

```
┌─────────────────────────────────────────────────────────┐
│                Browser Automation Platform               │
│             (AdsPower / Multilogin / custom)             │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │Profile 1 │  │Profile 2 │  │Profile N │              │
│  │ (Chrome) │  │ (Chrome) │  │ (Chrome) │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
│       │              │              │                     │
│  ┌────▼──────────────▼──────────────▼─────┐              │
│  │        CAPTCHA Detection Layer          │              │
│  │  (iframe detection, siteKey reading) │              │
│  └────────────────┬───────────────────────┘              │
│                   │                                      │
│  ┌────────────────▼───────────────────────┐              │
│  │        Human Verification Abstraction       │              │
│  │  ┌──────────┐ ┌──────────┐ ┌────────┐ │              │
│  │  │ Service  │ │ Service  │ │Service │ │              │
│  │  │ Router   │ │ Balancer │ │Fallback│ │              │
│  │  └────┬─────┘ └────┬─────┘ └───┬────┘ │              │
│  └───────┼────────────┼────────────┼──────┘              │
│          │            │            │                      │
└──────────┼────────────┼────────────┼──────────────────────┘
           │            │            │
     ┌─────▼──┐  ┌──────▼───┐  ┌────▼─────┐
     │2Captcha│  │CapSolver │  │Anti-     │
     │        │  │          │  │Captcha   │
     └────────┘  └──────────┘  └──────────┘
```

### Component Details

#### 1. CAPTCHA Detection Layer
```javascript
// Detect CAPTCHA type in page
const hasRecaptcha = await page.locator(
  'iframe[src*="recaptcha"], .g-recaptcha'
).count() > 0;

const hasTurnstile = await page.locator(
  'iframe[src*="challenges.cloudflare.com"], .cf-turnstile'
).count() > 0;

const hasHcaptcha = await page.locator(
  'iframe[src*="hcaptcha"], .h-captcha'
).count() > 0;

// Extract site key
const siteKey = await page.evaluate(() => {
  const el = document.querySelector('.g-recaptcha, .cf-turnstile, .h-captcha');
  return el?.getAttribute('data-sitekey') ||
         el?.getAttribute('data-size')?.sitekey;
});
```

#### 2. Token Injection
```javascript
// Inject reCAPTCHA token
await page.evaluate((token) => {
  document.querySelector('#g-recaptcha-response').innerHTML = token;
  if (typeof ___grecaptcha_cfg !== 'undefined') {
    // Trigger callback
    const clientId = document.querySelector('.g-recaptcha')?.dataset?.rcId;
    if (clientId) {
      grecaptcha.getResponse(clientId);
    }
  }
}, token);

// Inject Turnstile token
await page.evaluate((token) => {
  const widget = document.querySelector('.cf-turnstile');
  if (widget) widget.dispatchEvent(new CustomEvent('turnstile-callback', {
    detail: token
  }));
}, token);
```

#### 3. Solver Abstraction Layer (Pseudocode)

```go
type SolverClient interface {
  Solve(ctx context.Context, req *SolveRequest) (*SolveResponse, error)
  Balance() (float64, error)
  Name() string
}

type SolveRequest struct {
  Type      CaptchaType   // ImageToText, RecaptchaV2, Turnstile, etc.
  ImageBase64 string      // For image captchas
  SiteKey    string
  PageURL    string
  Proxy      *Proxy       // Optional
  Extra      map[string]any
}

type SolverManager struct {
  Primary   SolverClient
  Fallback  SolverClient
  HealthChecks []HealthCheck
}

func (m *SolverManager) Solve(ctx context.Context, req *SolveRequest) (*SolveResponse, error) {
  // Try primary with timeout
  resp, err := m.Primary.Solve(ctx, req)
  if err == nil { return resp, nil }

  // Fallback to alternative provider
  return m.Fallback.Solve(ctx, req)
}
```

#### 4. Session & Cookie Management

- **Profile-level state:** Each browser profile maintains its own session cookies
- **Token caching:** reCAPTCHA v3 tokens last ~120s; cache per site/profile
- **Cookie persistence:** Solved CAPTCHA state persists as long as session cookies remain valid
- **Warm-up:** For reCAPTCHA v3, browse to target site first (builds profile score), then execute

---

## 10. Retry Strategies & Cost Optimization

### Retry Strategy

```python
MAX_RETRIES = 3
BASE_DELAY = 2  # seconds

async def solve_with_retry(client, task):
    for attempt in range(MAX_RETRIES):
        try:
            result = await client.solve(task)

            # Validate result
            if is_valid_token(result):
                return result

            # Report bad solve to provider
            await client.report_incorrect(task.task_id)

        except TimeoutError:
            if attempt == MAX_RETRIES - 1:
                raise

        except RateLimitError:
            await asyncio.sleep(BASE_DELAY * (2 ** attempt))
            continue

        except InsufficientBalanceError:
            # Switch provider
            break

    # All retries exhausted → try fallback provider
    return await fallback_client.solve(task)
```

### Token Validation

```go
func isValidToken(token string, captchaType CaptchaType) bool {
  switch captchaType {
  case RecaptchaV2:
    return len(token) > 100 && strings.HasPrefix(token, "03AGdBq")
  case Turnstile:
    return len(token) > 50
  case ImageToText:
    return len(token) >= task.MinLength && len(token) <= task.MaxLength
  }
  return true
}
```

### Cost Optimization Strategies

| Strategy | Description | Savings |
|---|---|---|
| **Provider routing** | Route each CAPTCHA type to cheapest provider | 30-60% |
| **Report bad solves** | Most providers refund incorrect solves | Variable |
| **Batch image OCR** | Use local ddddocr for simple image CAPTCHA first | 100% (free) |
| **Token caching** | Cache reCAPTCHA v3 tokens (valid ~2 min) | Reduces duplicate solves |
| **Session reuse** | Keep browser sessions alive to avoid repeated CAPTCHAs | Depends on site |
| **Pingback mode** | Avoid polling (reduces API call costs) | Minimal |
| **Bulk deposit** | 2Captcha/Anti-Captcha offer volume discounts | 10-20% |
| **Balance monitoring** | Alert when balance < threshold to avoid rate limiting | Operational |

### Cost Calculation Example

For 10,000 reCAPTCHA v2 solves/day:

| Provider | Cost/1k | Daily Cost | Monthly Cost |
|---|---|---|---|
| 2Captcha | $2.99 | $29.90 | $897 |
| CapSolver | $1.50 | $15.00 | $450 |
| Anti-Captcha | $2.00 | $20.00 | $600 |
| CapMonster | $0.60 | $6.00 | $180 |
| **Smart router** | ~$1.00 | $10.00 | $300 |

### Provider Failover Priority (Recommended)

```
Level 1: ddddocr (free, image-only CAPTCHA) → success = no cost
Level 2: CapSolver or Anti-Captcha (fast AI) → cost-effective
Level 3: 2Captcha (highest reliability, broadest coverage) → fallback
```

### Rate Limiting Handling

| Provider | Limits | Strategy |
|---|---|---|
| **2Captcha** | No hard limit; quality degrades >100/min | Exponential backoff at 200+ req/min |
| **CapSolver** | ~50 req/min on free tier; higher with deposit | Monitor `errorId` responses |
| **Anti-Captcha** | "Theoretically unlimited" (~1,000/min) | Distribute across task pools |

### Go Implementation Sketch

```go
package captcha

import (
  "context"
  "time"
  "math/rand"
)

type ServiceType int
const (
  Service2Captcha ServiceType = iota
  ServiceCapSolver
  ServiceAntiCaptcha
)

type SolverConfig struct {
  ProviderOrder   []ServiceType
  Timeout         time.Duration   // default 120s
  PollInterval    time.Duration   // default 2s
  MaxRetries      int             // default 3
  FallbackOnError bool
  CacheTokens     bool
  CacheTTL        time.Duration   // default 60s
}

type TokenCache struct {
  data map[string]CacheEntry
}

type CacheEntry struct {
  Token     string
  ExpiresAt time.Time
}

type CloudflareTurnstileSolver struct {
  SiteKey string
  PageURL string
  Action  string
  Proxy   string
}

type GeetestV4Solver struct {
  CaptchaID string
  PageURL   string
  Proxy     string
}

type GeeTestV4Solution struct {
  CaptchaID string
  LotNumber string
  PassToken string
  GenTime   string
  CaptchaOutput string
}

type FunCaptchaSolution struct {
  Token string
}

type AmazonWafSolver struct {
  PageURL string
  SiteKey string
  Iv      string
  Context string
  ChallengeScript string
  CaptchaScript   string
}
```

---

## Quick Reference Card

### Which Service to Use For...

| Use Case | Recommendation |
|---|---|
| **Simple image CAPTCHA** (text-based) | ddddocr (free) → Anti-Captcha ImageToText ($0.50/1k) |
| **Chinese image CAPTCHA** | PaddleOCR → 2Captcha (human fallback) |
| **reCAPTCHA v2/v3** | CapSolver ($1.50/1k, fast) → 2Captcha (reliable) |
| **Cloudflare Turnstile** | CapSolver ($0.60/1k, best price) |
| **GeeTest v3/v4** | Anti-Captcha (best Go SDK) → 2Captcha |
| **FunCaptcha / Arkose** | 2Captcha (broadest coverage) |
| **Amazon WAF** | Anti-Captcha (Go SDK support) |
| **High-volume general** | CapMonster (cheapest AI, $0.60/1k) |
| **Go-based integration** | Anti-Captcha (`anticaptcha-go`) |
| **Rust-based integration** | ddddocr-rs + raw HTTP for cloud services |
| **Maximum reliability** | Aggregate with auto-failover across ≥2 providers |

### Quick API Reference

```
┌─────────────┬──────────────────────┬────────────────────────────┐
│  Service    │  Submit Endpoint     │  Retrieve Endpoint         │
├─────────────┼──────────────────────┼────────────────────────────┤
│  2Captcha   │ POST /in.php         │ GET /res.php               │
│  CapSolver  │ POST /createTask     │ POST /getTaskResult        │
│  AntiCap    │ POST /createTask     │ POST /getTaskResult        │
└─────────────┴──────────────────────┴────────────────────────────┘

All services use JSON for modern task types.
2Captcha legacy flow uses form-encoded in.php / res.php.
CapSolver and Anti-Captcha use REST JSON throughout.
```
