#!/bin/bash
# Record and convert the Purestat intro video
#
# Prerequisites:
#   - API running (cargo run -p purestat-api)
#   - Frontend running (cd ui && bun run dev)
#   - Playwright installed (cd e2e && bun install && bunx playwright install chromium)
#   - ffmpeg installed (for MP4 conversion)
#
# Usage:
#   ./scripts/record-video.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
E2E_DIR="$PROJECT_DIR/e2e"
OUTPUT_DIR="$PROJECT_DIR/e2e/video/output"

echo "=== Purestat Intro Video Recording ==="
echo ""

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Run the Playwright recording
echo "[1/3] Recording video with Playwright..."
cd "$E2E_DIR"
bunx playwright test video/record-intro.spec.ts --reporter=list 2>&1 || true

# Find the recorded video
WEBM_FILE=$(find "$E2E_DIR/test-results" -name "*.webm" -newer "$0" 2>/dev/null | head -1)

if [ -z "$WEBM_FILE" ]; then
  # Try broader search
  WEBM_FILE=$(find "$E2E_DIR/test-results" -name "*.webm" 2>/dev/null | sort -r | head -1)
fi

if [ -z "$WEBM_FILE" ]; then
  echo "ERROR: No video file found in test-results/"
  echo "Check that the test ran successfully and video recording is enabled."
  exit 1
fi

echo "[2/3] Found recording: $WEBM_FILE"

# Convert to MP4 with ffmpeg
MP4_FILE="$OUTPUT_DIR/purestat-intro.mp4"

if command -v ffmpeg &>/dev/null; then
  echo "[3/3] Converting to MP4..."
  ffmpeg -y -i "$WEBM_FILE" \
    -c:v libx264 \
    -preset slow \
    -crf 18 \
    -pix_fmt yuv420p \
    -movflags +faststart \
    "$MP4_FILE" 2>/dev/null

  echo ""
  echo "=== Done ==="
  echo "WebM: $WEBM_FILE"
  echo "MP4:  $MP4_FILE"
  echo ""
  echo "Upload to YouTube, then update README.md with the video link."
else
  echo "[3/3] ffmpeg not found — skipping MP4 conversion."
  echo ""
  echo "=== Done ==="
  echo "WebM: $WEBM_FILE"
  echo ""
  echo "To convert manually:"
  echo "  ffmpeg -i \"$WEBM_FILE\" -c:v libx264 -crf 18 -pix_fmt yuv420p \"$MP4_FILE\""
fi
