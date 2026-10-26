#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/validate-linux-block-io.sh --directory PATH [--size-mib N] [--monitorctl PATH]

Runs a bounded, disposable disk-I/O smoke test on the filesystem containing
PATH. Requires Bash, coreutils, findmnt, and an already-running
native-monitor-agent. Reads independent block counters from sysfs while the
agent reports `/proc/diskstats`-derived values. The temporary file is removed
on exit. No package installation or privilege escalation is attempted.

Limits: 1-256 MiB written once, then read once. Review the target filesystem
and available free space before running. This is a local trend/unit check, not
a capacity benchmark or proof of support on other platforms.
USAGE
}

directory=""
size_mib=64
monitorctl="${MONITORCTL:-target/debug/native-monitorctl}"

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
if [[ ! -x "$monitorctl" ]]; then
    printf 'monitorctl is not executable: %s\n' "$monitorctl" >&2
    exit 2
fi

directory="$(cd -- "$directory" && pwd -P)"
filesystem="$(findmnt -T "$directory" -n -o FSTYPE,SOURCE,TARGET)"
sample_directory="$(mktemp -d "${TMPDIR:-/tmp}/native-monitor-block-io.XXXXXXXX")"
sample_output="$sample_directory/sysfs-counters.tsv"
stop_marker="$sample_directory/stop"
temporary_file=""
sampler_pid=""
cleanup() {
    if [[ -n "$sampler_pid" ]] && kill -0 "$sampler_pid" 2>/dev/null; then
        kill "$sampler_pid" 2>/dev/null || true
        wait "$sampler_pid" 2>/dev/null || true
    fi
    if [[ -n "$temporary_file" ]]; then
        rm -f -- "$temporary_file"
    fi
    rm -f -- "$sample_output" "$stop_marker"
    rmdir -- "$sample_directory" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
temporary_file="$(mktemp "$directory/.native-monitor-io-check.XXXXXXXX")"

printf 'Target filesystem: %s\n' "$filesystem"
printf 'Writing and syncing at most %s MiB to a unique temporary file.\n' "$size_mib"
printf 'Independent read-only source: /sys/class/block/*/stat\n'
sample_sysfs_counters() {
    shopt -s nullglob
    printf 'timestamp_unix_ns\tdevice\tname\treads\tsectors_read\twrites\tsectors_written\tio_ms\n' > "$sample_output"
    while [[ ! -e "$stop_marker" ]]; do
        local timestamp_ns
        timestamp_ns="$(date +%s%N)"
        for stat_file in /sys/class/block/*/stat; do
            local device_path="${stat_file%/stat}"
            if [[ -e "$device_path/partition" ]]; then
                continue
            fi
            local slaves=("$device_path"/slaves/*)
            if ((${#slaves[@]} > 0)); then
                continue
            fi
            local device_id=""
            if ! IFS= read -r device_id < "$device_path/dev"; then
                continue
            fi
            local reads=0 reads_merged=0 sectors_read=0 read_ms=0
            local writes=0 writes_merged=0 sectors_written=0 write_ms=0
            local in_progress=0 io_ms=0 weighted_io_ms=0 discard_ios=0
            local discard_merged=0 discard_sectors=0 discard_ms=0 flush_ios=0 flush_ms=0
            IFS=' ' read -r reads reads_merged sectors_read read_ms writes writes_merged \
                sectors_written write_ms in_progress io_ms weighted_io_ms discard_ios \
                discard_merged discard_sectors discard_ms flush_ios flush_ms < "$stat_file"
            printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$timestamp_ns" "$device_id" "${device_path##*/}" "$reads" \
                "$sectors_read" "$writes" "$sectors_written" "$io_ms" >> "$sample_output"
        done
        sleep 0.25
    done
}
sample_sysfs_counters &
sampler_pid=$!

printf 'Agent snapshot before load:\n'
"$monitorctl" devices --type block

dd if=/dev/zero of="$temporary_file" bs=1M count="$size_mib" conv=fdatasync status=progress
dd if="$temporary_file" of=/dev/null bs=1M status=progress

printf 'Agent snapshots after load:\n'
for _ in 1 2 3; do
    "$monitorctl" devices --type block
    sleep 1
done

touch "$stop_marker"
wait "$sampler_pid"
sampler_pid=""
printf 'Independent sysfs counters observed during the test:\n'
cat "$sample_output"
printf 'Validation finished; temporary files will be removed. Match major:minor and compare counter deltas/trends with agent rates.\n'
