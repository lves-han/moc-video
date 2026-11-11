#!/usr/bin/env python3
import sys
from workflow import main

if __name__ == "__main__":
    sys.exit(main(["run", *sys.argv[1:]]))
