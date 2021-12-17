#!/usr/bin/env bash

set -euxo pipefail

bin_dir="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
project_dir="${bin_dir}/../.."

# Obtain Sparkle release
sparkle_tmp=$(mktemp -d)
cd "${sparkle_tmp}"
curl -L \
    https://github.com/sparkle-project/Sparkle/releases/download/2.0.0-rc.1/Sparkle-2.0.0-rc.1.tar.xz \
    | tar -xJ
echo "${SPARKLE_KEY}" > private_key

export PATH="${sparkle_tmp}/bin:$PATH"

# Clone the gh-pages
git_tmp=$(mktemp -d)
git clone --depth=1 --branch=gh-pages https://github.com/zhuhaow/Specht2 "${git_tmp}"

# Copy everything to release folder
release_tmp=$(mktemp -d)
cp "${project_dir}/app/Specht2.zip" "${release_tmp}/"
# The appcast file is not required
cp "${git_tmp}/appcast.xml" "${release_tmp}/" || true

generate_appcast \
    --download-url-prefix \
    "https://github.com/zhuhaow/Specht2/releases/download/${GITHUB_REF_NAME}/" \
    -f "${sparkle_tmp}/private_key" \
    "${release_tmp}"

cp "${release_tmp}/appcast.xml" "${git_tmp}/"
cd "${git_tmp}"
git add .
git commit -m "Update appcast for ${GITHUB_REF_NAME}"
git push --force --quite "https://${GITHUB_ACTOR}:${GITHUB_TOKEN}@github.com/zhuhaow/Specht2" gh-pages > /dev/null 2>&1
