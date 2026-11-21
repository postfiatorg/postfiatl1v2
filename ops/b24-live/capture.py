#!/usr/bin/env python3
"""Fresh Phase-5 capture through the qualified bounded runner; --execute required."""
import importlib.util
from pathlib import Path

path=Path(__file__).resolve().parents[1]/'b24-replacement/capture.py'
spec=importlib.util.spec_from_file_location('phase5_capture',path)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
module.OUT=module.COORD/'evidence/tee/phase5'
if __name__=='__main__':module.main()
