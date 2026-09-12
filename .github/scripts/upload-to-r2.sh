#!/bin/bash
# Upload build artifacts to Cloudflare R2
# Usage: upload-to-r2.sh <file_pattern> <platform> [arch]

set -e

FILE_PATTERN="$1"
PLATFORM="$2"
ARCH="${3:-}"
BUILD_DATE="${BUILD_DATE:-$(date +%Y%m%d)}"
VERSION="${VERSION:-1.4.6}"

# R2 configuration from environment
R2_BUCKET="${R2_BUCKET_NAME}"
R2_ENDPOINT="${R2_ENDPOINT_URL}"
R2_ACCESS_KEY="${R2_ACCESS_KEY_ID}"
R2_SECRET_KEY="${R2_SECRET_ACCESS_KEY}"

if [ -z "$R2_BUCKET" ] || [ -z "$R2_ENDPOINT" ]; then
    echo "Error: R2 credentials not configured"
    exit 1
fi

# Install AWS CLI if not present (for Linux)
if ! command -v aws &>/dev/null; then
    if command -v apt-get &>/dev/null; then
        apt-get update && apt-get install -y awscli
    elif command -v yum &>/dev/null; then
        yum install -y awscli
    fi
fi

# Upload files
for FILE in $FILE_PATTERN; do
    if [ -f "$FILE" ]; then
        FILENAME=$(basename "$FILE")
        # S3 path: VERSION/DATE/PLATFORM/FILENAME
        if [ -n "$ARCH" ]; then
            S3_PATH="${VERSION}/${BUILD_DATE}/${PLATFORM}/${ARCH}/${FILENAME}"
        else
            S3_PATH="${VERSION}/${BUILD_DATE}/${PLATFORM}/${FILENAME}"
        fi
        
        echo "Uploading $FILE to s3://${R2_BUCKET}/${S3_PATH}"
        
        aws s3 cp "$FILE" "s3://${R2_BUCKET}/${S3_PATH}" \
            --endpoint-url "$R2_ENDPOINT" \
            --region auto
        
        echo "Uploaded: ${S3_PATH}"
    fi
done

echo "Upload completed!"
