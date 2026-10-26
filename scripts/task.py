#!/usr/bin/env python3
import sys
from workflow import main

if __name__ == "__main__":
    sys.exit(main(["task", *sys.argv[1:]]))
