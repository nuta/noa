#!/usr/bin/env python3
import argparse
import subprocess
import tempfile

def run():
    with tempfile.NamedTemporaryFile() as tmp:
        p = subprocess.Popen(
            ["./target/debug/noa", "--headless"],
            stdin=subprocess.PIPE,
            bufsize=0,
        )
        p.stdin.write(b"abcdefghijk\x11")
        p.wait()

def main():
    parser = argparse.ArgumentParser()
    args = parser.parse_args()

    run()

if __name__ == "__main__":
    main()
