#!/bin/bash
# Render a local HTML file to PNG using browserless
set -e

BROWSERLESS_URL="${BROWSERLESS_URL:-http://localhost:3000}"
BROWSERLESS_TOKEN="${BROWSERLESS_TOKEN:-your-token}"

HTML_FILE="${1:?Usage: render-html.sh <file.html> [output.png]}"
OUTPUT="${2:-rendered.png}"

browser-cli screenshot --html "$HTML_FILE" -o "$OUTPUT" --width 1200 --height 800
echo "Rendered $HTML_FILE → $OUTPUT"
