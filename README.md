# 🌐 Browserless Kit

**Self-hosted headless Chrome with a Rust CLI and Cloudflare Tunnel — designed for AI assistants that need residential IP browser access.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## The Problem

Many services aggressively block datacenter/cloud IPs:
- **Fitness services** (Garmin, Strava, etc.) return HTTP 427/429 from cloud IPs
- **Banking sites** trigger CAPTCHAs or outright block
- **WAF-protected services** fingerprint and reject non-residential traffic

If your AI assistant runs in the cloud (AWS, GCP, Azure, or any AI platform), it **cannot** directly browse these services.

## The Solution

Run headless Chrome on your home NAS/server (residential IP) and tunnel it securely through Cloudflare:

```
┌─────────────────┐         ┌──────────────────┐         ┌─────────────────────┐
│  AI Assistant    │────────→│  Cloudflare      │────────→│  Home NAS / Server  │
│  (Cloud)         │         │  Tunnel          │         │                     │
│                  │         │  (encrypted)     │         │  ┌───────────────┐  │
│  browser-cli     │◄────────│                  │◄────────│  │  Browserless  │  │
│  (Rust binary)   │         │  Zero Trust      │         │  │  (Chrome)     │  │
│                  │         │  Access (opt.)   │         │  └───────────────┘  │
└─────────────────┘         └──────────────────┘         └─────────────────────┘
         │                                                         │
    Cloud/Datacenter IP                                   Residential IP
    (blocked by services)                                 (accepted everywhere)
```

Your AI assistant sends requests to the Rust CLI → routed through Cloudflare Tunnel → Chrome runs on your home network with a residential IP → services see a normal home user.

## Features

- 🏠 **Residential IP proxy** — browse any service through your home IP
- 🔧 **Rust CLI** — single binary, zero runtime dependencies
- 🔒 **Cloudflare Tunnel** — encrypted, no port forwarding, no VPN
- 🌐 **CDP WebSocket** — full Chrome DevTools Protocol for complex automation
- 📸 **Screenshots** — URL or local HTML → PNG
- 📄 **PDF generation** — any URL → PDF
- 🔍 **Content extraction** — HTML/text/markdown
- 🕷️ **Element scraping** — CSS selector-based extraction
- 🔐 **SSO automation** — automate SSO logins for various services via CDP
- 🍪 **Cookie sessions** — multi-step auth flows with persistent cookies
- 🏥 **Health monitoring** — check browserless container status

## Quick Start

### 1. Deploy Browserless Chrome

```bash
git clone https://github.com/rodrigobroilo/browserless-kit.git
cd browserless-kit

# Edit the token in docker-compose.yml
vim docker-compose.yml

# Start the container
docker-compose up -d

# Verify it's running
curl http://localhost:3000/pressure
```

### 2. Build the Rust CLI

```bash
cd cli
cargo build --release
# Binary at: target/release/browser-cli
```

### 3. Configure Environment

```bash
export BROWSERLESS_URL="http://localhost:3000"
export BROWSERLESS_TOKEN="your-token-here"

# If using Cloudflare Access (recommended):
export CF_ACCESS_CLIENT_ID="your-service-token-id"
export CF_ACCESS_CLIENT_SECRET="your-service-token-secret"
```

### 4. Take a Screenshot

```bash
browser-cli screenshot https://example.com -o example.png
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `browser-cli screenshot <url>` | Screenshot a URL → PNG |
| `browser-cli screenshot --html page.html` | Render local HTML → PNG |
| `browser-cli content <url>` | Extract page content (HTML/text/markdown) |
| `browser-cli pdf <url>` | Generate PDF from URL |
| `browser-cli scrape <url> -e "selector"` | Scrape elements by CSS selector |
| `browser-cli fetch <url>` | HTTP GET through Chrome (residential IP) |
| `browser-cli fetch <url> -m POST -b '{"data":1}'` | HTTP POST through Chrome |
| `browser-cli cdp script.jsonl` | Execute CDP automation script |
| `browser-cli proxy <url>` | Simple GET proxy through Chrome |
| `browser-cli health` | Check browserless container health |

### Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON (for programmatic use) |
| `--timeout <secs>` | Request timeout (default: 30s) |

### Screenshot Options

```bash
# Full page capture
browser-cli screenshot https://example.com --full-page -o full.png

# Custom viewport
browser-cli screenshot https://example.com --width 1920 --height 1080

# Wait for element before capture
browser-cli screenshot https://example.com --wait-for ".loaded"

# Delay before capture (ms)
browser-cli screenshot https://example.com --delay 2000
```

### PDF Options

```bash
# Landscape orientation
browser-cli pdf https://example.com --landscape -o report.pdf

# Letter format
browser-cli pdf https://example.com --format Letter
```

## Cloudflare Tunnel Setup

> See [cloudflare/TUNNEL_SETUP.md](cloudflare/TUNNEL_SETUP.md) for the detailed step-by-step guide.

### Quick Overview

1. Install `cloudflared` on your NAS/server
2. Create a tunnel: `cloudflared tunnel create browserless`
3. Configure ingress to point at `http://localhost:3000`
4. Set up DNS: CNAME your subdomain to the tunnel
5. **⚠️ CRITICAL: Set SSL/TLS mode to "Full"** (see below)

### ⚠️ The WebSocket Gotcha

**This is the #1 issue people hit.** If your Cloudflare domain's SSL/TLS mode is set to "Flexible" or "Off":

- HTTP/1.1 requests get a `301 Moved Permanently` redirect to `http://`
- This **breaks WebSocket upgrades** (CDP connections fail)
- HTTP/2 requests work fine, hiding the issue until you try WebSocket

**The fix:**

1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com) → select your domain
2. **SSL/TLS** → **Overview**
3. Change encryption mode to **"Full"** (or "Full (Strict)")

**Test it:**
```bash
# Should return "101 Switching Protocols"
curl -sv \
  -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  https://your-browserless.example.com/ 2>&1 | grep "HTTP/"

# If you see "301 Moved Permanently" → SSL mode is wrong
# If you see "101 Switching Protocols" → WebSocket works ✅
```

## CDP Automation (WebSocket)

The most powerful feature. Use CDP scripts for complex multi-step automation like SSO logins.

### Script Format

CDP scripts are JSON-lines files (one command per line):

```jsonl
{"method": "Page.navigate", "params": {"url": "https://example.com/login"}}
{"method": "wait", "params": {"ms": 2000}}
{"method": "wait_for_selector", "params": {"selector": "#username"}}
{"method": "type_text", "params": {"selector": "#username", "text": "user@example.com"}}
{"method": "type_text", "params": {"selector": "#password", "text": "secret"}}
{"method": "click", "params": {"selector": "#login-button"}}
{"method": "wait", "params": {"ms": 3000}}
{"method": "get_cookies", "params": {}}
{"method": "screenshot", "params": {"output": "after-login.png"}}
```

### Built-in Commands

| Command | Description |
|---------|-------------|
| `wait` | Sleep for N milliseconds |
| `wait_for_selector` | Wait for CSS selector to appear |
| `type_text` | Type text into an input field |
| `click` | Click an element by CSS selector |
| `get_cookies` | Get all cookies from the page |
| `screenshot` | Take a CDP screenshot |

Any other method is passed directly to Chrome DevTools Protocol (e.g., `Page.navigate`, `Runtime.evaluate`, `Network.enable`).

### Run a CDP Script

```bash
browser-cli cdp examples/login-flow.jsonl --cdp-timeout 60000
```

## Environment Variables

| Variable | Required | Description |
|----------|:--------:|-------------|
| `BROWSERLESS_URL` | ✅ | Browserless instance URL |
| `BROWSERLESS_TOKEN` | ✅ | Browserless auth token |
| `CF_ACCESS_CLIENT_ID` | ❌ | Cloudflare Access service token ID |
| `CF_ACCESS_CLIENT_SECRET` | ❌ | Cloudflare Access service token secret |
| `BROWSERLESS_HOST` | ❌ | Target hostname for proxy tunneling (CDP) |
| `HTTPS_PROXY` | ❌ | HTTP CONNECT proxy for WebSocket |

## Real-World Examples

### 1. Screenshot a Dashboard

```bash
browser-cli screenshot https://grafana.example.com/dashboard \
  --wait-for ".panel-container" \
  --delay 3000 \
  --full-page \
  -o dashboard.png
```

### 2. Render HTML Map to PNG

```bash
# Generate a Leaflet.js map HTML file, then render it
browser-cli screenshot --html flight-map.html -o map.png --width 1200 --height 800
```

### 3. Automate SSO Login via CDP

Use WebSocket CDP to automate multi-step login flows (banking, fitness services, corporate SSO, etc.):

```bash
# Connect via CDP WebSocket for interactive automation
browser-cli cdp https://sso.example.com/login \
  --script "fill('#username', 'user@example.com'); fill('#password', env('SSO_PASSWORD')); click('#submit');"
```

The flow:
1. Navigate to SSO login page → fill credentials → submit
2. Follow redirects → extract session cookies or service ticket
3. Exchange for API tokens
4. Save tokens locally for headless API access

### 4. Scrape WAF-Protected Site

```bash
# Through residential IP, bypasses datacenter blocks
browser-cli content https://protected-service.com --format text
```

### 5. Check Service Health

```bash
browser-cli health --json
# {"running":0,"waiting":0,"maxConcurrent":3,"maxQueued":5,...}
```

## For AI Assistants

This toolkit is specifically designed for AI assistants (like Claude, GPT, etc.) that run in cloud environments but need browser capabilities:

### Integration Pattern

```
AI Assistant
  └── browser-cli binary (Rust, ~3MB)
        └── Calls browserless over HTTPS
              └── Cloudflare Tunnel (encrypted)
                    └── Your home NAS (residential IP)
                          └── Chrome renders the page
```

### Why This Matters for AI

1. **Residential IP** — Services that block datacenter IPs (Garmin, banking, government sites) work through your home IP
2. **SSO Automation** — OAuth flows, MFA-less logins, cookie extraction — all via CDP WebSocket
3. **Rendering** — Turn HTML (maps, charts, reports) into PNGs for sharing
4. **Scraping** — Access WAF-protected sites that block simple HTTP requests
5. **Zero Dependencies** — Single Rust binary, no Python/Node runtime needed

### CDP Session Management

For complex flows (like SSO), the CDP WebSocket gives you full control:

1. **Create a target** (new browser tab)
2. **Navigate** to the login page
3. **Fill forms** programmatically
4. **Extract cookies/tokens** after authentication
5. **Reuse tokens** for subsequent API calls

The Rust CLI handles all WebSocket complexity, proxy tunneling, and TLS negotiation. Your AI just writes a JSON-lines script and runs it.

### Cookie Persistence

The `fetch` command supports cookie sessions for multi-step flows:

```bash
# Step 1: Login (cookies saved to session "myauth")
browser-cli fetch https://example.com/login \
  -m POST \
  -b '{"user":"me","pass":"secret"}' \
  --cookie-session myauth

# Step 2: Use authenticated session
browser-cli fetch https://example.com/api/data \
  --cookie-session myauth
```

## Docker Configuration

The included `docker-compose.yml` configures:

| Setting | Default | Description |
|---------|---------|-------------|
| `TOKEN` | (required) | Auth token for the API |
| `CONCURRENT` | 3 | Max concurrent Chrome sessions |
| `QUEUED` | 5 | Max queued requests |
| `TIMEOUT` | 60000 | Request timeout in ms |
| `HEALTH` | true | Enable health endpoint |
| Memory limit | 2GB | Docker memory cap |

Adjust based on your NAS/server resources. Each Chrome session uses ~150-300MB RAM.

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| `301 Moved Permanently` on WebSocket | SSL/TLS mode is "Flexible" or "Off" | Change to "Full" in Cloudflare Dashboard |
| `403 Forbidden` | CF Access blocking request | Add service token headers (`CF_ACCESS_CLIENT_ID`, `CF_ACCESS_CLIENT_SECRET`) |
| `Timeout` | Page too slow or container overloaded | Increase `--timeout` or `TIMEOUT` env var |
| `WebSocket closed unexpectedly` | Container ran out of memory | Increase Docker memory limit |
| `Connection refused` | Container not running | `docker-compose up -d` |
| CDP script hangs | Wrong selector or page didn't load | Add `wait` commands, check selectors |

## Architecture

```
browser-cli (Rust binary)
├── client.rs    — HTTP client with CF-Access headers, token management
├── commands.rs  — Screenshot, content, PDF, scrape, fetch, proxy, health
├── cdp.rs       — WebSocket CDP automation (proxy tunneling, TLS, session management)
└── main.rs      — CLI argument parsing (clap)
```

The binary handles:
- **HTTP CONNECT proxy tunneling** for WebSocket through corporate/cloud proxies
- **TLS certificate verification bypass** for MITM proxies (common in cloud environments)
- **Cloudflare Access headers** injection on every request
- **Token-based auth** via query parameter

## Contributing

Issues and PRs welcome! This project grew out of real-world needs — if you're building AI assistants that need browser access, we'd love to hear about your use case.

## License

[MIT](LICENSE)
