#!/usr/bin/env python3
import argparse
import subprocess
import tempfile

def run(bin):
    with tempfile.NamedTemporaryFile() as tmp:
        p = subprocess.Popen(
            [bin, "--headless"],
            stdin=subprocess.PIPE,
            bufsize=0,
        )
        p.stdin.write(b"abcdefghijk\x11")
        p.wait()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", default="./target/debug/noa")
    args = parser.parse_args()

    run(args.bin)

if __name__ == "__main__":
    main()
