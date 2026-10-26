#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/validate-linux-block-io.sh --directory PATH [--size-mib N] [--monitorctl PATH]

Runs a bounded, disposable disk-I/O smoke test on the filesystem containing
PATH. Requires sysstat's iostat and an already-running native-monitor-agent.
The temporary file is removed on exit. No package installation or privilege
escalation is attempted.

Limits: 1-256 MiB written once, then read once. Review the target filesystem
and available free space before running. This is a local trend/unit check, not
a capacity benchmark or proof of support on other platforms.
USAGE
}

directory=""
size_mib=64
monitorctl="${MONITORCTL:-target/debug/monitorctl}"

while (($# > 0)); do
    case "$1" in
        --directory)
            (($# >= 2)) || { usage >&2; exit 2; }
            directory="$2"
            shift 2
            ;;
        --size-mib)
            (($# >= 2)) || { usage >&2; exit 2; }
            size_mib="$2"
            shift 2
            ;;
        --monitorctl)
            (($# >= 2)) || { usage >&2; exit 2; }
            monitorctl="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ "$(uname -s)" != Linux ]]; then
    printf 'This validation is Linux-only.\n' >&2
    exit 2
fi
if [[ -z "$directory" || ! -d "$directory" ]]; then
    printf 'Provide an existing --directory on the filesystem to test.\n' >&2
    usage >&2
    exit 2
fi
if ! [[ "$size_mib" =~ ^[0-9]+$ ]] || ((size_mib < 1 || size_mib > 256)); then
    printf '%s\n' '--size-mib must be an integer from 1 to 256.' >&2
    exit 2
fi
if ! command -v iostat >/dev/null 2>&1; then
    printf 'Missing iostat (sysstat); install it using your distribution package manager, then retry.\n' >&2
    exit 2
fi
if [[ ! -x "$monitorctl" ]]; then
    printf 'monitorctl is not executable: %s\n' "$monitorctl" >&2
    exit 2
fi

directory="$(cd -- "$directory" && pwd -P)"
filesystem="$(findmnt -T "$directory" -n -o FSTYPE,SOURCE,TARGET)"
temporary_file="$(mktemp "$directory/.native-monitor-io-check.XXXXXXXX")"
iostat_pid=""
cleanup() {
    if [[ -n "$iostat_pid" ]] && kill -0 "$iostat_pid" 2>/dev/null; then
        kill "$iostat_pid" 2>/dev/null || true
        wait "$iostat_pid" 2>/dev/null || true
    fi
    rm -f -- "$temporary_file"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Target filesystem: %s\n' "$filesystem"
printf 'Writing and syncing at most %s MiB to a unique temporary file.\n' "$size_mib"
printf 'Independent iostat sample (device counters):\n'
iostat -dx 1 2 &
iostat_pid=$!

printf 'Agent snapshot before load:\n'
"$monitorctl" devices --type block

dd if=/dev/zero of="$temporary_file" bs=1M count="$size_mib" conv=fdatasync status=progress
dd if="$temporary_file" of=/dev/null bs=1M status=progress

printf 'Agent snapshots after load:\n'
for _ in 1 2 3; do
    "$monitorctl" devices --type block
    sleep 1
done

wait "$iostat_pid"
iostat_pid=""
printf 'Validation finished; temporary file will be removed. Compare device deltas/trends and units in both outputs.\n'
