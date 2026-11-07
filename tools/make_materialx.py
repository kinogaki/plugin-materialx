#!/usr/bin/env python3
"""MaterialX's own Python module for tools/materialx_oracle.py: a virtual
environment in FOLDER holding MaterialX 1.39.5, the release whose libraries
the SDK vendors (sdk/crates/mtlx/libraries), installed from PyPI's wheel for
macOS on Apple silicon and refused unless its SHA-256 is one pinned here.

    python3 tools/make_materialx.py FOLDER
    FOLDER/bin/python3 tools/materialx_oracle.py tests/fixtures/examples tests/fixtures/written

Kinogaki Studio's tools/fetch_data.py makes it in ~/.kinogaki/data/materialx,
where its test runner finds it.
"""
import argparse
import pathlib
import subprocess
import tempfile
import venv

VERSION = '1.39.5'
# PyPI's SHA-256 of each of MaterialX 1.39.5's macOS arm64 wheels, CPython
# 3.9 to 3.14: pip installs the one for the Python running this, or none.
WHEELS = [
    '9eba6994b5262e91e5b21d54112ad802fdb963a9928c2d8f73242474076b2f1e',  # cp39
    '2434b2ad758cd78d21ef90be8fdf303e65ee7eb1ddc9c5b473e1013588da3907',  # cp310
    '7b530c0f660590c8f63ae2b7d598130ac046e52643dc0baa761f288fd3748840',  # cp311
    '58b0f3d9ce2171e71733e0a0e5b1170869a4d0eb273a9ed07fa91479d13552e2',  # cp312
    '8c2d33025010495ec6dcea0dd33b2ec0b5a4d1563e42a6bff89db980bfb92b93',  # cp313
    'c444fe85e6e62147a9e13339db0a8657d0ca554b19719ff35735a201cbe4b604',  # cp314
]

parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument('folder', type=pathlib.Path, help='the virtual environment to make')
folder = parser.parse_args().folder
# Used through its bin/python3 alone, which finds the environment around it
# wherever the folder is moved (fetch_data.py renames it into place).
venv.create(folder, with_pip=True)
python = str(folder / 'bin' / 'python3')
with tempfile.TemporaryDirectory() as scratch:
    pinned = pathlib.Path(scratch) / 'requirements.txt'
    pinned.write_text(f'MaterialX=={VERSION} ' + ' '.join(f'--hash=sha256:{h}' for h in WHEELS) + '\n')
    subprocess.run([python, '-m', 'pip', 'install', '--require-hashes', '--only-binary=:all:', '--no-deps',
                    '--no-cache-dir', '--disable-pip-version-check', '--no-input', '-r', str(pinned)], check=True)
found = subprocess.run([python, '-I', '-c', 'import MaterialX; print(MaterialX.getVersionString())'],
                       capture_output=True, text=True, check=True).stdout.strip()
if found != VERSION:
    raise SystemExit(f'{folder}: MaterialX {found}, not {VERSION}')
print(f'{python} (MaterialX {found})')
