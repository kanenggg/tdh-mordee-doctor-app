#!/bin/bash
# Test script for Patient Past Visit API endpoint
# Usage: ./test_past_visit.sh [options]

set -e

# Default values
BASE_URL="${BASE_URL:-http://localhost:8081}"
PATIENT_ACCOUNT_ID="${PATIENT_ACCOUNT_ID:-232}"
PATIENT_PROFILE_ID="${PATIENT_PROFILE_ID:-232}"
DOCTOR_ACCOUNT_ID="${DOCTOR_ACCOUNT_ID:-1}"
ACCOUNT_TYPE="${ACCOUNT_TYPE:-2}"  # 2 = doctor, 4 = backoffice
EMAIL="${EMAIL:-test@example.com}"
PRETTY="${PRETTY:-0}"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --pretty)
      PRETTY=1
      shift
      ;;
    --backoffice)
      ACCOUNT_TYPE=4
      shift
      ;;
    --patient-id)
      PATIENT_ACCOUNT_ID="$2"
      PATIENT_PROFILE_ID="$2"
      shift 2
      ;;
    --url)
      BASE_URL="$2"
      shift 2
      ;;
    -h|--help)
      echo "Usage: $0 [options]"
      echo ""
      echo "Options:"
      echo "  --pretty            Pretty-print JSON response with jq"
      echo "  --backoffice        Use backoffice account type instead of doctor"
      echo "  --patient-id ID     Set patient account/profile ID (default: 232)"
      echo "  --url URL           Set base URL (default: http://localhost:8081)"
      echo "  -h, --help          Show this help message"
      echo ""
      echo "Environment Variables:"
      echo "  BASE_URL            Base URL for the API"
      echo "  PATIENT_ACCOUNT_ID  Patient account ID"
      echo "  PATIENT_PROFILE_ID  Patient profile ID"
      echo "  DOCTOR_ACCOUNT_ID   Doctor account ID for auth header"
      echo "  ACCOUNT_TYPE        Account type (2=doctor, 4=backoffice)"
      echo "  EMAIL               Email for auth header"
      echo "  PRETTY              Set to 1 to enable pretty-printing"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use -h or --help for usage information"
      exit 1
      ;;
  esac
done

# Build user identity JSON
USER_IDENTITY=$(cat <<EOF
{"doctor_account_id": ${DOCTOR_ACCOUNT_ID}, "account_type": ${ACCOUNT_TYPE}, "email": "${EMAIL}"}
EOF
)

URL="${BASE_URL}/ehr/v1/past-visit?patientAccountId=${PATIENT_ACCOUNT_ID}&patientProfileId=${PATIENT_PROFILE_ID}"

echo "Testing Past Visit API endpoint..."
echo "URL: ${URL}"
echo "Patient Account ID: ${PATIENT_ACCOUNT_ID}"
echo "Patient Profile ID: ${PATIENT_PROFILE_ID}"
echo "Account Type: $([ "$ACCOUNT_TYPE" -eq 2 ] && echo 'Doctor' || echo 'Backoffice')"
echo ""

# Execute
CURL_ARGS=(
  curl -X GET "${URL}"
  -H "tdh-sec-iam-user-identity: ${USER_IDENTITY}"
)

if [ "$PRETTY" -eq 1 ]; then
  "${CURL_ARGS[@]}" -s | jq .
else
  "${CURL_ARGS[@]}" -v
fi
