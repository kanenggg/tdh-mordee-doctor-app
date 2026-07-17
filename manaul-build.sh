sha=$(git rev-parse --short HEAD)
datetime=$(date +%Y%m%d%H%M%S)
tag="$sha-$datetime"
gcloud builds submit \
    --tag "asia-southeast1-docker.pkg.dev/tdg-dh-truehealth-core-nonprod/cossack-docker/doctorapp-server:$tag" \
    "server" \
    --machine-type="e2-highcpu-8"