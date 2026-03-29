#!/bin/bash
# Example: CDP automation to login to a website via WebSocket
#
# This demonstrates the JSON-lines script format for browser-cli cdp.
# Modify the selectors and URLs for your target service.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Create a temporary CDP script
cat > /tmp/cdp-login-example.jsonl << 'EOF'
{"method": "Page.navigate", "params": {"url": "https://example.com/login"}}
{"method": "wait", "params": {"ms": 2000}}
{"method": "wait_for_selector", "params": {"selector": "#username", "timeout": 10000}}
{"method": "type_text", "params": {"selector": "#username", "text": "your-username"}}
{"method": "type_text", "params": {"selector": "#password", "text": "your-password"}}
{"method": "click", "params": {"selector": "button[type='submit']"}}
{"method": "wait", "params": {"ms": 3000}}
{"method": "get_cookies", "params": {}}
{"method": "screenshot", "params": {"output": "/tmp/login-result.png"}}
EOF

echo "Running CDP login automation..."
browser-cli cdp /tmp/cdp-login-example.jsonl --cdp-timeout 60000

echo "Done! Check /tmp/login-result.png for the result."
rm -f /tmp/cdp-login-example.jsonl
