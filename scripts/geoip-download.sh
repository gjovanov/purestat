#!/bin/sh
set -e

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1; shift ;;
    *) break ;;
  esac
done

DB_PATH="${PURESTAT__GEO__GEOIP_DB_PATH:-/data/geoip/GeoLite2-City.mmdb}"
DB_DIR="$(dirname "$DB_PATH")"

if [ -z "$MAXMIND_ACCOUNT_ID" ] || [ -z "$MAXMIND_LICENSE_KEY" ]; then
  echo "[geoip] MAXMIND_ACCOUNT_ID or MAXMIND_LICENSE_KEY not set — skipping GeoIP download"
else
  if [ "$FORCE" = "1" ] || [ ! -f "$DB_PATH" ]; then
    echo "[geoip] Downloading GeoLite2-City.mmdb ..."
    mkdir -p "$DB_DIR"
    TMP_DIR=$(mktemp -d)
    curl -sS -L \
      -u "${MAXMIND_ACCOUNT_ID}:${MAXMIND_LICENSE_KEY}" \
      "https://download.maxmind.com/geoip/databases/GeoLite2-City/download?suffix=tar.gz" \
      -o "${TMP_DIR}/GeoLite2-City.tar.gz"
    tar -xzf "${TMP_DIR}/GeoLite2-City.tar.gz" -C "$TMP_DIR"
    MMDB_FILE=$(find "$TMP_DIR" -name "GeoLite2-City.mmdb" -type f | head -1)
    if [ -z "$MMDB_FILE" ]; then
      echo "[geoip] ERROR: GeoLite2-City.mmdb not found in archive"
      rm -rf "$TMP_DIR"
      exit 1
    fi
    mv "$MMDB_FILE" "$DB_PATH"
    rm -rf "$TMP_DIR"
    echo "[geoip] Installed $(ls -lh "$DB_PATH" | awk '{print $5}') -> $DB_PATH"
  else
    echo "[geoip] $DB_PATH already exists — skipping (use --force to re-download)"
  fi
fi

exec "$@"
