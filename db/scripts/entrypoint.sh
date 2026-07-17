#!/bin/sh
# shellcheck disable=SC2164
cd "$HOME"

if [ -z "${PROJECT_NAME}" ]; then
  echo "PROJECT_NAME env variable is not set"
  exit 1
fi

if [ -z "${SCRIPT_DIR}" ]; then
  export SCRIPT_DIR=db;
fi

if [ -z "${TEST_LOCAL}" ]; then
  TEST_LOCAL=0
fi

export PATH=$PATH:$HOME/bin
export GIT_FILE_PATH=$HOME/db-script

if [ "$TEST_LOCAL" -ne 1 ]; then
  git-clone-db-script.sh;
  ret_val=$?
  if [ "$ret_val" -ne 0 ]; then
    exit 1;
  fi
fi

if [ -z "$ENTRY_POINT_SCRIPT" ]; then
  ENTRY_POINT_SCRIPT="entrypoint.sh";
fi

ENTRY_POINT=${GIT_FILE_PATH}/projects/${PROJECT_NAME}/${SCRIPT_DIR}/${ENTRY_POINT_SCRIPT}

if [ ! -f "$ENTRY_POINT" ]
then
  echo "The entrypoint script file is not not found: $ENTRY_POINT"
  echo "Please create the script file name 'entrypoint.sh' under the directory 'projects/${PROJECT_NAME}/${SCRIPT_DIR}' in your git repository."
  exit 1
fi

cd "$GIT_FILE_PATH"/projects/"$PROJECT_NAME"/"$SCRIPT_DIR"

# shellcheck disable=SC2035
chmod +x *.sh
chmod +x sql/*.sh

$ENTRY_POINT