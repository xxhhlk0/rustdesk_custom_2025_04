#!/bin/bash
# Upload files to Cloudflare R2
# Usage: r2-upload.sh <local_path> <r2_path> [file_pattern]
#
# Examples:
#   r2-upload.sh ./build/ builds/123/windows/
#   r2-upload.sh ./dist/ builds/123/libs/ "*.so"

set -e

LOCAL_PATH="$1"
R2_PATH="$2"
FILE_PATTERN="${3:-*}"

if [ -z "$LOCAL_PATH" ] || [ -z "$R2_PATH" ]; then
    echo "Usage: r2-upload.sh <local_path> <r2_path> [file_pattern]"
    exit 1
fi

# R2 configuration from environment
R2_BUCKET="${R2_BUCKET_NAME}"
R2_ENDPOINT="${R2_ENDPOINT_URL}"

if [ -z "$R2_BUCKET" ] || [ -z "$R2_ENDPOINT" ]; then
    echo "Error: R2_BUCKET_NAME and R2_ENDPOINT_URL must be set"
    exit 1
fi

# Remove leading slash from R2_PATH
R2_PATH="${R2_PATH#/}"

echo "Uploading to R2: s3://${R2_BUCKET}/${R2_PATH}"

# Handle directory or single file
if [ -d "$LOCAL_PATH" ]; then
    # Upload directory contents
    for FILE in "$LOCAL_PATH"/$FILE_PATTERN; do
        if [ -f "$FILE" ]; then
            FILENAME=$(basename "$FILE")
            echo "Uploading: $FILE -> ${R2_PATH}${FILENAME}"
            aws s3 cp "$FILE" "s3://${R2_BUCKET}/${R2_PATH}${FILENAME}" \
                --endpoint-url "$R2_ENDPOINT" \
                --region auto
        fi
    done
elif [ -f "$LOCAL_PATH" ]; then
    # Upload single file
    FILENAME=$(basename "$LOCAL_PATH")
    echo "Uploading: $LOCAL_PATH -> ${R2_PATH}${FILENAME}"
    aws s3 cp "$LOCAL_PATH" "s3://${R2_BUCKET}/${R2_PATH}${FILENAME}" \
        --endpoint-url "$R2_ENDPOINT" \
        --region auto
else
    echo "Error: $LOCAL_PATH not found"
    exit 1
fi

echo "Upload completed!"
