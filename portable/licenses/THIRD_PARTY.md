# Third-party components

The application and native image renderer use the root MIT license. The Windows setup wrapper retains the original KohdaLab IV MIT notice at native/KOHDALAB-IV-LICENSE.

Bundled libwdi retains its LGPL terms, source, notices and replaceable DLL. Its source/license files are in native/vendor/libwdi and the portable's support/SOURCE.zip. Python, NumPy, Pillow, OpenCV, PySide6/Qt, PyUSB, libusb and PyInstaller retain their upstream terms. Component notices present in the bundled runtimes must be retained. This document does not relicense third-party code.

Manufacturer driver installers, MeasureS and GR300W binaries downloaded for analysis are not distributed.

The build copies installed component license files and records exact versions in `versions.json`. Qt/PySide6 6.11.2 license texts are included in `Qt/`; sources are available from https://download.qt.io/archive/qt/6.11/6.11.2/submodules/ and https://download.qt.io/official_releases/QtForPython/pyside6/PySide6-6.11.2-src/ . Qt/PySide libraries remain separately replaceable within the bundled shared runtime; the application source and rebuild scripts are supplied in SOURCE.zip. Modified versions of these libraries may be used; reverse engineering for debugging those modifications is permitted under their license terms.
