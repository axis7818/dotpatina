#!/usr/bin/env sh

prev_ref=${1:-refs/remotes/origin/main}

PREV_VERSION=$(git show $prev_ref:Cargo.toml | grep '^version' | awk '{print $3}')
if [ -z "$PREV_VERSION" ]; then
    echo "❌ Previous version not found."
    exit 1
fi
echo "Previous version = ${PREV_VERSION}"

CURR_VERSION=$(grep '^version' Cargo.toml | awk '{print $3}')
if [ -z "$CURR_VERSION" ]; then
    echo "❌ Current version not found."
    exit 1
fi
echo "Current version = ${CURR_VERSION}"

if [ "$PREV_VERSION" = "$CURR_VERSION" ]; then
    echo "❌ Cargo version has not incremented."
    exit 1
fi

echo "✅ Cargo version has changed"
