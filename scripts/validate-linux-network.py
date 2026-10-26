#!/usr/bin/env python3
"""Compare Linux loopback network counters with the monitor agent locally."""

from __future__ import annotations

import argparse
import os
import pathlib
import platform
import socket
import subprocess
import threading
import time


def read_loopback_counters() -> tuple[int, int]:
    with pathlib.Path("/proc/net/dev").open(encoding="ascii") as counters_file:
        for line in counters_file:
            if ":" not in line:
                continue
            interface, values = line.split(":", 1)
            if interface.strip() == "lo":
                counters = [int(value) for value in values.split()]
                return counters[0], counters[8]
    raise RuntimeError("loopback interface 'lo' was not found in /proc/net/dev")


def run_monitorctl(monitorctl: str) -> None:
    subprocess.run([monitorctl, "devices", "--type", "network"], check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--size-mib", type=int, default=8, help="loopback payload size (1-32 MiB; default: 8)"
    )
    parser.add_argument(
        "--monitorctl", default="target/debug/native-monitorctl", help="monitorctl executable"
    )
    args = parser.parse_args()

    if platform.system() != "Linux":
        parser.error("this validation requires Linux /proc/net/dev")
    if not 1 <= args.size_mib <= 32:
        parser.error("--size-mib must be between 1 and 32")
    monitorctl_path = pathlib.Path(args.monitorctl)
    if not monitorctl_path.is_file() or not os.access(monitorctl_path, os.X_OK):
        parser.error(f"monitorctl executable not found or not executable: {args.monitorctl}")

    status = subprocess.run(
        [args.monitorctl, "status"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    if status.returncode != 0:
        parser.error("a reachable native-monitor-agent is required; this script will not start or stop it")

    payload_size = args.size_mib * 1024 * 1024
    received = 0
    receiver_error: list[Exception] = []

    def receive_payload(listener: socket.socket) -> None:
        nonlocal received
        try:
            connection, _ = listener.accept()
            with connection:
                while received < payload_size:
                    chunk = connection.recv(min(64 * 1024, payload_size - received))
                    if not chunk:
                        raise RuntimeError("loopback sender closed before the full payload arrived")
                    received += len(chunk)
        except Exception as error:  # propagate the helper thread's failure to main
            receiver_error.append(error)

    before_rx, before_tx = read_loopback_counters()
    print(f"Loopback /proc/net/dev before: RX={before_rx} B TX={before_tx} B")
    print("Agent snapshot before traffic:")
    run_monitorctl(args.monitorctl)

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.settimeout(15)
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        receiver = threading.Thread(target=receive_payload, args=(listener,), daemon=True)
        receiver.start()

        def send_payload() -> None:
            try:
                with socket.create_connection(listener.getsockname(), timeout=5) as connection:
                    remaining = payload_size
                    block = b"n" * (64 * 1024)
                    while remaining > 0:
                        chunk_size = min(len(block), remaining)
                        connection.sendall(block[:chunk_size])
                        remaining -= chunk_size
                        time.sleep(0.01)
            except Exception as error:
                receiver_error.append(error)

        sender = threading.Thread(target=send_payload, daemon=True)
        sender.start()
        while sender.is_alive() or receiver.is_alive():
            sender.join(timeout=1)
            receiver.join(timeout=0.1)
            if sender.is_alive() or receiver.is_alive():
                print("Agent snapshot during loopback traffic:")
                run_monitorctl(args.monitorctl)
        sender.join()
        receiver.join()

    if receiver_error:
        raise RuntimeError(f"loopback transfer failed: {receiver_error[0]}")
    if received != payload_size:
        raise RuntimeError(f"received {received} of {payload_size} expected bytes")

    after_rx, after_tx = read_loopback_counters()
    print(
        "Loopback /proc/net/dev after: "
        f"RX delta={after_rx - before_rx} B TX delta={after_tx - before_tx} B"
    )
    print("Agent snapshot after traffic:")
    run_monitorctl(args.monitorctl)
    print(
        f"Transferred {received} bytes over 127.0.0.1. Compare these independent "
        "counter deltas with the timestamped agent rate snapshots; this is a "
        "bounded local trend check, not a throughput benchmark."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
