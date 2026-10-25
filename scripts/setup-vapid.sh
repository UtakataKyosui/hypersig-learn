#!/bin/sh
# Sets up the VAPID identity of the worker.
#
#   scripts/setup-vapid.sh [--dry-run] [EMAIL]
#   scripts/setup-vapid.sh --subject-only [EMAIL]
#
# Generates a P-256 key pair and stores the public key, the private key, and
# the contact address as the GitHub secrets VAPID_PUBLIC_KEY, VAPID_PRIVATE_KEY,
# and VAPID_SUBJECT, which the deploy workflow binds to the worker. It also
# writes them to .dev.vars for `cf dev`. GitHub secrets cannot be read
# back, so keep a copy of the private key somewhere safe. It refuses to run when
# VAPID_PRIVATE_KEY already exists, because a new key invalidates every push
# subscription. --subject-only changes the address alone. --dry-run generates a
# key and prints both halves, touching nothing.
set -eu

cd "$(dirname "$0")/.."

mode=setup
email=
for arg in "$@"; do
    case "$arg" in
        --dry-run) mode=dry-run ;;
        --subject-only) mode='subject-only' ;;
        -*) echo "unknown option: $arg" >&2; exit 2 ;;
        *) email=$arg ;;
    esac
done

if [ -z "$email" ]; then
    if [ "$mode" = dry-run ]; then
        email=dry-run@example.com
    else
        printf 'Contact email for push services: ' >&2
        read -r email
    fi
fi
valid=$(printf '%s\n' "$email" | sed -En '/^[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$/p')
if [ -z "$valid" ]; then
    echo "not a valid email address" >&2
    exit 2
fi
subject="mailto:$email"

b64url() {
    openssl base64 -A | tr '+/' '-_' | tr -d '='
}

# Replaces the line `$1=...` in .dev.vars and keeps every other line.
set_dev_var() {
    umask 077
    touch .dev.vars
    sed "/^$1=/d" .dev.vars > .dev.vars.tmp
    printf '%s=%s\n' "$1" "$2" >> .dev.vars.tmp
    mv .dev.vars.tmp .dev.vars
}

set_secret() {
    printf '%s' "$2" | gh secret set "$1" > /dev/null
}

if [ "$mode" != dry-run ]; then
    secrets=$(gh secret list) || {
        echo "could not list GitHub secrets: run 'gh auth login' and check the repository" >&2
        exit 1
    }
    has_key=false
    case "$secrets" in VAPID_PRIVATE_KEY* | *"
VAPID_PRIVATE_KEY"*) has_key=true ;; esac
    if [ "$mode" = setup ] && [ "$has_key" = true ]; then
        echo "VAPID_PRIVATE_KEY already exists; a new key would invalidate every subscription." >&2
        echo "Use --subject-only to change the contact address." >&2
        exit 1
    fi
    if [ "$mode" = 'subject-only' ] && [ "$has_key" = false ]; then
        echo "VAPID_PRIVATE_KEY does not exist yet; run without --subject-only." >&2
        exit 1
    fi
fi

if [ "$mode" != 'subject-only' ]; then
    # A SEC1 ECPrivateKey in DER: the 32-byte private scalar starts at byte 8
    # and the 65-byte uncompressed public point is the last 65 bytes.
    der=$(mktemp)
    trap 'rm -f "$der"' EXIT
    openssl ecparam -name prime256v1 -genkey -noout -outform DER > "$der"
    if [ "$(wc -c < "$der" | tr -d ' ')" != 121 ]; then
        echo "unexpected key encoding from openssl" >&2
        exit 1
    fi
    private=$(tail -c +8 "$der" | head -c 32 | b64url)
    public=$(tail -c 65 "$der" | b64url)
fi

case "$mode" in
    dry-run)
        printf 'public  %s\n' "$public"
        printf 'private %s\n' "$private"
        ;;
    subject-only)
        set_dev_var VAPID_SUBJECT "$subject"
        set_secret VAPID_SUBJECT "$subject"
        echo "VAPID_SUBJECT updated. It reaches the worker on the next deploy."
        ;;
    setup)
        # The private key goes last: its presence marks a finished setup, so
        # an interrupted run can be run again.
        set_dev_var VAPID_PUBLIC_KEY "$public"
        set_dev_var VAPID_PRIVATE_KEY "$private"
        set_dev_var VAPID_SUBJECT "$subject"
        set_secret VAPID_PUBLIC_KEY "$public"
        set_secret VAPID_SUBJECT "$subject"
        set_secret VAPID_PRIVATE_KEY "$private"
        echo "VAPID_PUBLIC_KEY, VAPID_PRIVATE_KEY and VAPID_SUBJECT stored."
        echo "They reach the worker on the next deploy."
        ;;
esac
