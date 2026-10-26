#!/usr/bin/env bash
set -euo pipefail

if (($# != 0)); then
    printf 'Usage: scripts/test-victoriametrics-remote-write.sh\n' >&2
    exit 2
fi
for command_name in cargo curl docker; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Required command is unavailable: %s\n' "$command_name" >&2
        exit 2
    fi
done
if ! docker compose version >/dev/null 2>&1; then
    printf 'Docker Compose plugin is required.\n' >&2
    exit 2
fi

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "$script_directory/.." && pwd -P)"
compose_file="$repository_root/tests/observability/docker-compose.yml"
project_name="native-monitor-vm-rw-$RANDOM-$$"
vm_port="${VM_PORT:-18428}"
if ! [[ "$vm_port" =~ ^[0-9]{1,5}$ ]] || ((10#$vm_port < 1 || 10#$vm_port > 65535)); then
    printf 'VM_PORT must be an integer from 1 to 65535.\n' >&2
    exit 2
fi
base_url="http://127.0.0.1:$vm_port"
config_file="$(mktemp "${TMPDIR:-/tmp}/native-monitor-rw.XXXXXXXX.toml")"
log_file="${config_file%.toml}.log"
agent_pid=""
compose_started=0

cleanup() {
    if [[ -n "$agent_pid" ]]; then
        kill -INT "$agent_pid" 2>/dev/null || true
        wait "$agent_pid" 2>/dev/null || true
    fi
    if ((compose_started)); then
        docker compose --project-name "$project_name" --file "$compose_file" down --remove-orphans
    fi
    rm -f -- "$config_file" "$log_file"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

(cd "$repository_root" && cargo build -p native-monitor-agent -p native-monitorctl)
if "$repository_root/target/debug/native-monitorctl" status >/dev/null 2>&1; then
    printf 'An agent is already active for this user; refusing to interfere with it.\n' >&2
    exit 1
fi

compose_started=1
VM_PORT="$vm_port" docker compose \
    --project-name "$project_name" \
    --file "$compose_file" \
    up --detach

ready=0
for _ in $(seq 1 40); do
    if curl --silent --fail --max-time 1 "$base_url/health" >/dev/null 2>&1; then
        ready=1
        break
    fi
    sleep 0.5
done
if ((ready == 0)); then
    printf 'VictoriaMetrics did not become healthy within 20 seconds.\n' >&2
    exit 1
fi

cat > "$config_file" <<TOML
[export.remote_write]
enabled = true
url = "http://127.0.0.1:${vm_port}/api/v1/write"
interval_ms = 1000
families = ["cpu"]
max_memory_queue_mib = 1
TOML

"$repository_root/target/debug/native-monitor-agent" run "$config_file" >"$log_file" 2>&1 &
agent_pid=$!

found=0
for _ in $(seq 1 30); do
    now="$(date -u +%s)"
    start="$((now - 3600))"
    end="$((now + 3600))"
    response="$(curl --silent --fail --max-time 2 --get \
        --data-urlencode 'match[]=native_monitor_cpu_utilization_ratio' \
        --data-urlencode "start=$start" \
        --data-urlencode "end=$end" \
        "$base_url/api/v1/series" 2>/dev/null || true)"
    if [[ "$response" == *'native_monitor_cpu_utilization_ratio'* ]]; then
        found=1
        break
    fi
    if ! kill -0 "$agent_pid" 2>/dev/null; then
        printf 'Agent exited before VictoriaMetrics indexed a CPU series:\n' >&2
        cat "$log_file" >&2
        exit 1
    fi
    sleep 1
done

if ((found == 0)); then
    printf 'VictoriaMetrics did not index the expected Remote Write CPU series.\n' >&2
    printf 'Agent log (no endpoint credentials are configured by this test):\n' >&2
    cat "$log_file" >&2
    exit 1
fi

printf 'VictoriaMetrics indexed native_monitor_cpu_utilization_ratio via Remote Write v1.\n'
