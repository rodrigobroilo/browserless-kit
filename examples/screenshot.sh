#!/bin/bash
# Take a screenshot of any URL through your browserless instance
set -e

BROWSERLESS_URL="${BROWSERLESS_URL:-http://localhost:3000}"
BROWSERLESS_TOKEN="${BROWSERLESS_TOKEN:-your-token}"

URL="${1:?Usage: screenshot.sh <url> [output.png]}"
OUTPUT="${2:-screenshot.png}"

browser-cli screenshot "$URL" -o "$OUTPUT"
echo "Screenshot saved to $OUTPUT"
