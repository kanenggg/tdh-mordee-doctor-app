#!/bin/sh

# Clone a git repository and execute a script from within it.
#
# Usage: run-from-repo.sh
#
# Required env vars:
#   GIT_REPO          - Git repository URL (SSH or HTTPS)
#   GIT_BRANCH        - Branch to clone
#   ENTRY_POINT_FILE  - Relative path to the script inside the repo
#                       (e.g. scripts/onboard.sh). Must be executable (chmod +x).
#
# Optional SSH key (for SSH-based git URLs):
#   Mount the SSH private key secret to $HOME/ssh-key/ (i.e. /home/no-user/ssh-key/).
#   The script copies id_rsa to ~/.ssh/ and writes an SSH config with
#   StrictHostKeyChecking=no so host verification never blocks the clone.

_fail() {
	echo "$1"
	exit 1
}

[ -z "${GIT_REPO}" ] && _fail "GIT_REPO env variable is not set"
[ -z "${GIT_BRANCH}" ] && _fail "GIT_BRANCH env variable is not set"
[ -z "${ENTRY_POINT_FILE}" ] && _fail "ENTRY_POINT_FILE env variable is not set"

# ---------------------------------------------------------------------------
# SSH setup (runs only when key is mounted at $HOME/ssh-key/id_rsa)
# ---------------------------------------------------------------------------
SSH_KEY_DIR="${HOME}/ssh-key"

if [ -f "${SSH_KEY_DIR}/id_rsa" ]; then
	echo ">>> Setting up SSH key..."

	mkdir -p "${HOME}/.ssh"
	cp "${SSH_KEY_DIR}/id_rsa" "${HOME}/.ssh/id_rsa"
	chmod 600 "${HOME}/.ssh/id_rsa"

	# Write SSH client config.
	# StrictHostKeyChecking=no  — skips known_hosts verification; safe for CI
	#                             where the remote URL is operator-controlled.
	# UserKnownHostsFile=/dev/null — prevents stale entries causing failures.
	# IdentityFile — points explicitly to the mounted key.
	cat >"${HOME}/.ssh/config" <<EOF
Host *
  StrictHostKeyChecking no
  UserKnownHostsFile /dev/null
  IdentityFile ${HOME}/.ssh/id_rsa
EOF
	chmod 600 "${HOME}/.ssh/config"
fi

# ---------------------------------------------------------------------------
# Clone
# ---------------------------------------------------------------------------
WORK_DIR=$(mktemp -d)

echo ">>> Cloning ${GIT_REPO} (branch: ${GIT_BRANCH})..."
git clone --depth 1 -b "${GIT_BRANCH}" "${GIT_REPO}" "${WORK_DIR}"
ret_val=$?
if [ "$ret_val" -ne 0 ]; then
	_fail "Failed to clone repository"
fi

# ---------------------------------------------------------------------------
# Execute entrypoint
# ---------------------------------------------------------------------------
SCRIPT="${WORK_DIR}/${ENTRY_POINT_FILE}"

if [ ! -f "${SCRIPT}" ]; then
	_fail "Entrypoint not found in repo: ${ENTRY_POINT_FILE}"
fi

if [ ! -x "${SCRIPT}" ]; then
	_fail "Entrypoint is not executable: ${ENTRY_POINT_FILE} (run: chmod +x ${ENTRY_POINT_FILE})"
fi

echo ">>> Running ${ENTRY_POINT_FILE}..."
exec "${SCRIPT}"
