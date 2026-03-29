# Cloudflare Tunnel Setup for Browserless

Complete step-by-step guide to expose your local Browserless Chrome instance securely through Cloudflare Tunnel.

## Prerequisites

- A Cloudflare account (free tier works)
- A domain managed by Cloudflare DNS
- Docker running on your NAS/server
- `cloudflared` installed on your NAS/server

## Step 1: Install cloudflared

### Docker (recommended for NAS)
```yaml
# Add to your docker-compose.yml
cloudflared:
  image: cloudflare/cloudflared:latest
  container_name: cloudflared
  restart: unless-stopped
  command: tunnel run
  volumes:
    - ./cloudflare:/etc/cloudflared
```

### Linux
```bash
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 -o /usr/local/bin/cloudflared
chmod +x /usr/local/bin/cloudflared
```

### macOS
```bash
brew install cloudflared
```

## Step 2: Authenticate

```bash
cloudflared tunnel login
# Opens browser → select your domain → authorizes cloudflared
# Creates ~/.cloudflared/cert.pem
```

## Step 3: Create Tunnel

```bash
cloudflared tunnel create browserless
# Output: Created tunnel browserless with id <TUNNEL-UUID>
```

## Step 4: Configure Tunnel

Create `config.yml`:

```yaml
tunnel: <TUNNEL-UUID>
credentials-file: /etc/cloudflared/<TUNNEL-UUID>.json

ingress:
  - hostname: browserless.yourdomain.com
    service: http://localhost:3000
  - service: http_status:404
```

### For Docker networking

If browserless runs in a separate Docker container, use the container name:

```yaml
ingress:
  - hostname: browserless.yourdomain.com
    service: http://browserless:3000
  - service: http_status:404
```

Or if using host networking / Docker bridge:

```yaml
ingress:
  - hostname: browserless.yourdomain.com
    service: http://host.docker.internal:3000
  - service: http_status:404
```

## Step 5: DNS Setup

```bash
cloudflared tunnel route dns browserless browserless.yourdomain.com
# Creates a CNAME record: browserless.yourdomain.com → <TUNNEL-UUID>.cfargotunnel.com
```

## Step 6: Start the Tunnel

```bash
cloudflared tunnel run browserless
```

Or via Docker (if using docker-compose):
```bash
docker-compose up -d cloudflared
```

## Step 7: ⚠️ CRITICAL — SSL/TLS Configuration

**This is the most important step. Skip it and WebSocket (CDP) will NOT work.**

1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com)
2. Select your domain
3. Navigate to **SSL/TLS** → **Overview** (left sidebar)
4. Set encryption mode to **"Full"** or **"Full (Strict)"**

### Why This Matters

When SSL/TLS is set to "Flexible" or "Off":
- Cloudflare sends a `301 Moved Permanently` redirect for HTTP/1.1 requests
- Location header points to `http://` (downgrade)
- WebSocket upgrade handshake uses HTTP/1.1 → gets redirected → **breaks**
- HTTP/2 requests work fine (hiding the issue for normal browsing)

With SSL/TLS set to "Full":
- No redirect happens
- WebSocket upgrade succeeds → `101 Switching Protocols`
- CDP connections work through the tunnel

### Verification

```bash
# Test WebSocket upgrade
curl -sv \
  -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  https://browserless.yourdomain.com/ 2>&1 | grep "HTTP/"

# ✅ Good: "< HTTP/1.1 101 Switching Protocols"
# ❌ Bad:  "< HTTP/1.1 301 Moved Permanently"
```

## Step 8: (Optional) Cloudflare Access

Add Zero Trust authentication to restrict who can use your browserless instance.

### Create a Service Token

1. Go to [Cloudflare Zero Trust](https://one.dash.cloudflare.com)
2. **Access** → **Service Auth** → **Service Tokens**
3. Create a token → save the Client ID and Client Secret

### Create an Access Application

1. **Access** → **Applications** → **Add an Application**
2. Type: Self-hosted
3. Name: Browserless
4. Domain: `browserless.yourdomain.com`
5. Add a policy:
   - Name: `Service Token`
   - Action: `Service Auth`
   - Include: Service Token = your token

### Using with browser-cli

```bash
export CF_ACCESS_CLIENT_ID="your-service-token-client-id"
export CF_ACCESS_CLIENT_SECRET="your-service-token-client-secret"
```

The CLI automatically includes these headers on every request.

## Full docker-compose.yml Example

```yaml
version: "3"
services:
  browserless:
    image: ghcr.io/browserless/chromium:latest
    container_name: browserless
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      - TOKEN=your-browserless-token-here
      - CONCURRENT=3
      - QUEUED=5
      - TIMEOUT=60000
      - HEALTH=true
    deploy:
      resources:
        limits:
          memory: 2G

  cloudflared:
    image: cloudflare/cloudflared:latest
    container_name: cloudflared
    restart: unless-stopped
    command: tunnel run
    volumes:
      - ./cloudflare:/etc/cloudflared
    depends_on:
      - browserless
```

## Troubleshooting

### Tunnel shows "healthy" but no WebSocket
→ SSL/TLS mode is wrong. Set to "Full".

### 502 Bad Gateway
→ Browserless container isn't running or isn't on port 3000. Check `docker ps`.

### CF Access returns 403 even with service token
→ Verify headers are included:
```bash
curl -H "CF-Access-Client-Id: YOUR_ID" \
     -H "CF-Access-Client-Secret: YOUR_SECRET" \
     https://browserless.yourdomain.com/pressure
```

### WebSocket works locally but not through tunnel
→ Make sure the ingress hostname matches your DNS exactly. Check `cloudflared tunnel info browserless`.
