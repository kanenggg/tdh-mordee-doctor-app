#!/bin/sh

if [ -z "${GIT_REPO}" ];
then
  echo "GIT_REPO env variable is not set.";
  exit 1;
fi

if [ -z "${GIT_BRANCH}" ];
then
  echo "GIT_BRANCH env variable is not set.";
  exit 1;
fi

if [ -z "${PROJECT_NAME}" ]; then
  echo "PROJECT_NAME env variable is not set"
  exit 1
fi


if [ -z "${SCRIPT_DIR}" ];
then
  echo "SCRIPT_DIR env variable is not set.";
  exit 1;
fi

if [ -z "${GIT_FILE_PATH}" ];
then
  echo "GIT_FILE_PATH env variable is not set.";
  exit 1;
fi

if [ ! -f "$HOME"/ssh-key/id_rsa ];
then
  echo "No 'id_rsa' file found in $HOME/ssh-key"
  exit 1
fi

GIT_HOST=$(echo "$GIT_REPO" | sed -e 's/^.*@//g'| sed -e 's/:.*$//g')
mkdir "$HOME"/.ssh
ssh-keyscan "$GIT_HOST" >> "$HOME"/.ssh/known_hosts
cp "$HOME"/ssh-key/* "$HOME"/.ssh
chmod 400 "$HOME"/.ssh/id_rsa

git init "$GIT_FILE_PATH"
# shellcheck disable=SC2164
cd "$GIT_FILE_PATH"
git remote add -f origin "$GIT_REPO"

git config core.sparseCheckout true
echo "projects/$PROJECT_NAME/$SCRIPT_DIR" >> .git/info/sparse-checkout
git pull origin "$GIT_BRANCH"
