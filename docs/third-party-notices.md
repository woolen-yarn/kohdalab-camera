# Static InstallShield decoding references

`scripts/extract_measure_setup.py` uses the InstallShield format and nibble/XOR decoding described by:

- ISx, copyright (c) 2017 lifenjoiner — https://github.com/lifenjoiner/ISx
- ISSetupStream extractor, copyright (c) 2019 Mantas Mikulenas
- Unpacker — https://github.com/pawstas80/Unpacker

MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

No vendor application, driver binary, or vendor source code is redistributed in this repository.

## KohdaLab IV UI reference

The camera UI theme and panel arrangement are adapted from `src/kohdalab_iv/apps/iv_gui.py` in https://github.com/woolen-yarn/kohdalab-iv (main, retrieved 2026-10-06).

MIT License

Copyright (c) 2026 KohdaLab

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Windows camera setup

The dedicated WinUSB setup wrapper and rebuild script are adapted from KohdaLab IV (MIT); see native/KOHDALAB-IV-LICENSE. libwdi is dynamically linked and distributed with its corresponding source and LGPL license in native/vendor/libwdi and support/SOURCE.zip. Its DLL can be replaced with an ABI-compatible modified build. Microsoft inbox WinUSB is supplied by Windows, not redistributed as a custom kernel driver.
