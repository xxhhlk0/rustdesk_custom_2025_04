#!/bin/bash
# Download files from Cloudflare R2
# Usage: r2-download.sh <r2_path> <local_path> [file_pattern]
#
# Examples:
#   r2-download.sh builds/123/bridge/ ./
#   r2-download.sh builds/123/libs/ ./libs/ "*.so"

set -e

R2_PATH="$1"
LOCAL_PATH="$2"
FILE_PATTERN="${3:-*}"

if [ -z "$R2_PATH" ] || [ -z "$LOCAL_PATH" ]; then
    echo "Usage: r2-download.sh <r2_path> <local_path> [file_pattern]"
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

# Create local directory if needed
mkdir -p "$LOCAL_PATH"

echo "Downloading from R2: s3://${R2_BUCKET}/${R2_PATH}"

# List and download files
FILES=$(aws s3 ls "s3://${R2_BUCKET}/${R2_PATH}" --endpoint-url "$R2_ENDPOINT" --region auto | awk '{print $4}' || true)

if [ -z "$FILES" ]; then
    echo "No files found at s3://${R2_BUCKET}/${R2_PATH}"
    exit 1
fi

for FILE in $FILES; do
    # Check if file matches pattern
    if [[ "$FILE" == $FILE_PATTERN ]]; then
        echo "Downloading: ${R2_PATH}${FILE} -> ${LOCAL_PATH}${FILE}"
        aws s3 cp "s3://${R2_BUCKET}/${R2_PATH}${FILE}" "${LOCAL_PATH}${FILE}" \
            --endpoint-url "$R2_ENDPOINT" \
            --region auto
    fi
done

echo "Download completed!"
