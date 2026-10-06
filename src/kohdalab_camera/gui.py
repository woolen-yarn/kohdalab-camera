"""Capture I/O lives in one worker; UI polls only the latest owned frame."""
from queue import Queue, Empty
from threading import Lock
import time
import sys
import os
from pathlib import Path

from PySide6.QtCore import QThread, Signal, QTimer, Qt, QProcess
from PySide6.QtGui import QImage, QPixmap, QFont, QFontDatabase
from PySide6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QPushButton, QComboBox, QSpinBox, QDoubleSpinBox, QLineEdit, QFileDialog, QTabWidget, QFormLayout, QCheckBox, QGroupBox, QPlainTextEdit, QAbstractSpinBox, QSizePolicy, QScrollArea, QFrame)

from . import __version__
from .backends import make_camera,available_backends
from .storage import save_frame


class CaptureWorker(QThread):
    status = Signal(str)
    ready = Signal()

    def __init__(self, camera):
        super().__init__()
        self.camera = camera
        self.commands = Queue(maxsize=16)
        self.lock = Lock()
        self.latest = None
        self.recoveries = 0

    def submit(self, command):
        self.commands.put_nowait(command)

    def snapshot(self):
        with self.lock:
            return self.latest

    def run(self):
        try:
            self.camera.open()
            self.ready.emit()
            while not self.isInterruptionRequested():
                try:
                    command = self.commands.get_nowait()
                except Empty:
                    command = None
                if command:
                    try:
                        if command[0] == "control":
                            self.status.emit(str(self.camera.set_control(command[1], command[2])))
                        elif command[0] == "controls":
                            self.status.emit("Applying camera controls…")
                            values=command[1]
                            if hasattr(self.camera,"set_controls"):
                                result=self.camera.set_controls(values)
                            else:
                                result={name:self.camera.set_control(name,value) for name,value in values.items()}
                            self.status.emit("Camera controls applied.")
                        elif command[0] == "save-full":
                            self.status.emit("Capturing full sensor image…")
                            frame=self.camera.capture_full_frame(self.isInterruptionRequested)
                            self.status.emit(f"Saved full sensor image: {save_frame(frame,command[1],command[2])}")
                        elif command[0] == "save":
                            frame = self.snapshot()
                            if frame is None:
                                raise RuntimeError("No frame yet")
                            self.status.emit(f"Saved: {save_frame(frame, command[1], command[2])}")
                        elif command[0]=='image':
                            self.camera.set_image_options(command[1])
                        elif command[0]=='display':
                            self.camera.set_display_mode(command[1])
                        elif command[0]=='gamma':
                            self.camera.set_display_gamma(command[1])
                        elif command[0]=='white':
                            frame=self.snapshot()
                            if frame is None:raise RuntimeError('No frame yet.')
                            self.camera.balance_white(frame.sensor_raw)
                            self.status.emit('White balance applied.')
                        elif command[0]=='white-reset':
                            self.camera.reset_white_balance()
                        elif command[0]=='fault':
                            self.camera.inject_transport_fault()
                    except Exception as exc:
                        self.status.emit(f"Operation failed: {exc}")
                if hasattr(self.camera,'read_interruptible'):
                    frame=self.camera.read_interruptible(self.isInterruptionRequested)
                else:frame = self.camera.read()
                recovered=frame.controls.get('usb_recoveries',0)
                if recovered>self.recoveries:
                    self.status.emit('USB capture recovered. Camera restored to 128 rows / 1× gain; use Capture brightness.' if frame.controls.get('rejected_hardware_profile') else f'Stream resumed; camera settings retained ({recovered}).')
                    self.recoveries=recovered
                with self.lock:
                    self.latest = frame
        except InterruptedError:
            self.status.emit('Capture stopped.')
        except Exception as exc:
            self.status.emit(f"Capture stopped: {exc}")
        finally:
            try:
                self.camera.close()
            except Exception as exc:
                self.status.emit(f"Cleanup failed: {exc}")


class CameraWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        if sys.platform == 'win32':
            # Qt's offscreen plugin doesn't discover Windows system fonts.
            font = Path(os.environ.get('WINDIR', r'C:\Windows')) / 'Fonts' / 'segoeui.ttf'
            if font.is_file():
                QFontDatabase.addApplicationFont(str(font))
                self.setFont(QFont('Segoe UI', 9))
        self.setWindowTitle(f"KohdaLab Camera {__version__}")
        QApplication.instance().setStyle("Fusion")
        self.resize(1240, 760)
        self.worker = None
        self.close_pending = False
        self.last_backend = None
        self.started_at = 0
        self.displayed = 0
        root = QWidget(); self.setCentralWidget(root)
        outer = QHBoxLayout(root); outer.setContentsMargins(8,8,8,8); outer.setSpacing(8)
        center=QWidget(); layout=QVBoxLayout(center); layout.setContentsMargins(0,0,0,0); layout.setSpacing(8)
        outer.addWidget(center,1)
        run=QGroupBox("Camera"); top=QHBoxLayout(run); layout.addWidget(run)
        self.start_button=QPushButton("Connect"); self.stop_button=QPushButton("Disconnect")
        self.save_button=QPushButton("Capture image"); self.save_button.setObjectName("capture")
        for w in (self.start_button,self.stop_button,self.save_button):top.addWidget(w)
        body=QHBoxLayout(); layout.addLayout(body,1)
        self.preview=QLabel("Connect a camera to start live view"); self.preview.setObjectName("preview")
        self.preview.setAlignment(Qt.AlignmentFlag.AlignCenter); self.preview.setMinimumSize(400,300)
        body.addWidget(self.preview,1)
        self.tabs=QTabWidget(); self.tabs.setMaximumWidth(360); self.tabs.setMinimumWidth(330)
        left=QWidget(); left_layout=QHBoxLayout(left); left_layout.setContentsMargins(0,0,0,0); left_layout.setSpacing(4)
        left_layout.addWidget(self.tabs); self.panel_toggle=QPushButton("‹"); self.panel_toggle.setFixedWidth(22)
        self.panel_toggle.setToolTip("Show or hide controls"); left_layout.addWidget(self.panel_toggle)
        outer.insertWidget(0,left)
        def toggle_controls():
            shown=not self.tabs.isVisible(); self.tabs.setVisible(shown); self.panel_toggle.setText("‹" if shown else "›")
        self.panel_toggle.clicked.connect(toggle_controls)
        def panel(name):
            page=QWidget(); form=QFormLayout(page); form.setContentsMargins(8,12,8,8); form.setSpacing(12); form.setFieldGrowthPolicy(QFormLayout.FieldGrowthPolicy.AllNonFixedFieldsGrow); form.setRowWrapPolicy(QFormLayout.RowWrapPolicy.WrapLongRows)
            form.setAlignment(Qt.AlignmentFlag.AlignTop)
            scroll=QScrollArea(); scroll.setWidgetResizable(True); scroll.setFrameShape(QFrame.Shape.NoFrame)
            scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff); scroll.setWidget(page)
            self.tabs.addTab(scroll,name); return form
        capture=panel("Capture"); image=panel("Image"); export=panel("Export"); advanced=panel("Advanced"); self.advanced_form=advanced
        self.unit_label=QLabel(); self.unit_label.setWordWrap(True); capture.addRow(self.unit_label)
        self.exposure=QDoubleSpinBox(); self.gain=QDoubleSpinBox(); self.auto_exposure=QDoubleSpinBox()
        self.exposure.setValue(10); self.gain.setValue(1)
        advanced.addRow("Exposure (rows)",self.exposure); advanced.addRow("Analog gain",self.gain)
        self.apply_button=QPushButton("Apply camera controls"); advanced.addRow(self.apply_button)
        self.color_mode=QComboBox(); self.color_mode.addItems(["Color","Monochrome"]); image.addRow("Mode",self.color_mode)
        self.brightness=QDoubleSpinBox(); self.brightness.setRange(-3,4); self.brightness.setSingleStep(.25); self.brightness.setSuffix(" EV"); self.brightness.setValue(1.5)
        self.brightness.setToolTip("Software brightness for preview and RGB export; sensor data stays unchanged.")
        capture.addRow("Brightness",self.brightness)
        self.tone_button=QPushButton("Auto brightness"); capture.addRow(self.tone_button)
        self.gamma=QDoubleSpinBox(); self.gamma.setRange(.5,3); self.gamma.setDecimals(1); self.gamma.setValue(2.2); image.addRow("Gamma",self.gamma)
        self.mirror=QCheckBox("Mirror horizontally"); self.flip=QCheckBox("Flip vertically")
        image.addRow(self.mirror); image.addRow(self.flip)
        image.addRow(QLabel("Orientation: 180°"))
        self.white_button=QPushButton("Set white balance"); self.white_button.setToolTip("Fill the image with a neutral white or gray reference first.")
        self.white_reset_button=QPushButton("Reset white balance"); image.addRow(self.white_button); image.addRow(self.white_reset_button)
        self.directory=QLineEdit("captures"); export.addRow("Destination",self.directory)
        browse=QPushButton("Choose folder…"); export.addRow(browse)
        self.format=QComboBox(); self.format.addItems(["png","tiff"]); export.addRow("Format",self.format)
        self.full_save_button=QPushButton("Capture full sensor · 3.1 MP")
        self.full_save_button.setToolTip("Temporarily switch to 2048 × 1536, save a still image, then resume low-bandwidth live view.")
        export.addRow(self.full_save_button)
        note=QLabel("Save at the active camera resolution. RGB image, capture metadata and original sensor TIFF are saved together."); note.setWordWrap(True); export.addRow(note)
        self.capture_profile=QComboBox()
        self.capture_profile.addItem("Low bandwidth · 1024 × 768","balanced")
        self.capture_profile.addItem("Full sensor · 2048 × 1536","full")
        self.capture_profile.setToolTip("Choose before connecting. Full sensor mode prioritizes resolution; live mode prioritizes frame rate.")
        advanced.addRow("Camera mode",self.capture_profile)
        self.backend=QComboBox(); self.backend.addItems([b.id for b in available_backends()])
        self.api=QComboBox(); self.api.addItems(["auto","avfoundation","msmf","dshow"])
        self.index=QSpinBox(); self.index.setRange(0,31)
        advanced.addRow("Backend",self.backend); advanced.addRow("OS API",self.api); advanced.addRow("Device index",self.index)
        self.auto_button=QPushButton("Apply auto-exposure value"); advanced.addRow("API auto exposure",self.auto_exposure); advanced.addRow(self.auto_button)
        self.stats=QLabel("Disconnected"); layout.addWidget(self.stats)
        self.message=QLabel("Ready. Brightness is in Capture; orientation and color are in Image."); self.message.setWordWrap(True); layout.addWidget(self.message)
        self.log=QPlainTextEdit(); self.log.setReadOnly(True); self.log.setMaximumBlockCount(200)
        self.log.setMinimumWidth(190); self.log.setMaximumWidth(230); self.log.setPlaceholderText("Capture log")
        self.log.setVisible(False)
        self.log_toggle=QPushButton("‹"); self.log_toggle.setFixedWidth(22); self.log_toggle.setToolTip("Show or hide capture log")
        outer.addWidget(self.log_toggle); outer.addWidget(self.log)
        def toggle_log():
            shown=not self.log.isVisible(); self.log.setVisible(shown); self.log_toggle.setText("›" if shown else "‹")
        self.log_toggle.clicked.connect(toggle_log)
        self.setStyleSheet("""
            QWidget { background:#050505; color:#e8e8e8; font-size:10pt; }
            QGroupBox { border:1px solid #333; border-radius:4px; margin-top:14px; padding:8px; background:#0b0b0b; }
            QGroupBox::title { subcontrol-origin:margin; left:8px; padding:0 4px; color:#f0f0f0; }
            QLabel#preview { background:#050505; border:1px solid #333; border-radius:4px; color:#999; }
            QTabWidget::pane { border:1px solid #333; border-radius:4px; background:#0b0b0b; }
            QTabBar::tab { padding:7px 6px; background:#0b0b0b; color:#aaa; border:1px solid #333; }
            QTabBar::tab:selected { color:#e8e8e8; border-bottom:2px solid #5aa9ff; }
            QLineEdit,QComboBox,QDoubleSpinBox,QSpinBox,QPlainTextEdit { background:#111; border:1px solid #3a3a3a; border-radius:3px; color:#f2f2f2; selection-background-color:#255d8f; padding:6px; }
            QComboBox QAbstractItemView { background:#181818; color:#f2f2f2; selection-background-color:#255d8f; selection-color:white; min-width:220px; padding:4px; }
            QComboBox QAbstractItemView::item { min-height:28px; padding:5px 10px; }
            QPushButton { background:#1b1b1b; border:1px solid #4a4a4a; border-radius:3px; color:#f0f0f0; padding:4px 8px; }
            QPushButton:hover { background:#2a2a2a; } QPushButton:disabled { color:#777; background:#121212; }
            QCheckBox { padding:3px; } QToolTip { background:#1b1b1b; color:#f0f0f0; }
        """)
        for spin in (self.exposure,self.gain,self.gamma,self.brightness,self.auto_exposure):
            spin.setKeyboardTracking(False)
            spin.setButtonSymbols(QAbstractSpinBox.ButtonSymbols.NoButtons)
        for combo in (self.backend,self.api,self.format,self.color_mode,self.capture_profile):
            combo.setMinimumWidth(140); combo.setMinimumHeight(30)
            combo.setSizePolicy(QSizePolicy.Policy.Expanding,QSizePolicy.Policy.Fixed)
            combo.view().setMinimumWidth(220)
        self.capture_profile.view().setMinimumWidth(300)
        self.start_button.clicked.connect(self.start_capture)
        self.stop_button.clicked.connect(self.stop_capture)
        self.apply_button.clicked.connect(self.apply_controls)
        self.auto_button.clicked.connect(lambda: self.enqueue(("control", "auto_exposure", self.auto_exposure.value())))
        self.full_save_button.clicked.connect(lambda:self.enqueue(("save-full",self.directory.text(),self.format.currentText())))
        self.save_button.clicked.connect(lambda: self.enqueue(("save", self.directory.text(), self.format.currentText())))
        browse.clicked.connect(self.choose_directory)
        self.backend.currentTextChanged.connect(self.update_units)
        self.color_mode.currentTextChanged.connect(lambda text:self.enqueue(('display','color' if text=='Color' else 'gray')))
        self.gamma.valueChanged.connect(lambda value:self.enqueue(('gamma',value)))
        self.white_button.clicked.connect(lambda:self.enqueue(('white',)))
        self.white_reset_button.clicked.connect(lambda:self.enqueue(('white-reset',)))
        for spin in (self.brightness,):spin.valueChanged.connect(lambda *_:self.enqueue(('image',self.image_values())))
        for check in (self.mirror,self.flip):check.toggled.connect(lambda *_:self.enqueue(('image',self.image_values())))
        self.tone_button.clicked.connect(self.auto_tone)
        self.timer = QTimer(self)
        self.timer.setTimerType(Qt.TimerType.PreciseTimer)
        self.timer.setInterval(16)
        self.timer.timeout.connect(self.refresh)
        self.timer.start()
        self.update_units()
        self.set_connected(False)

    def update_units(self):
        simulated = self.backend.currentText() == "simulated"
        legacy = self.backend.currentText() == "legacy-tca"
        changed=self.backend.currentText()!=self.last_backend
        if self.worker is None and legacy:
            self.exposure.setRange(1,1536)
            self.gain.setRange(1,8)
            self.exposure.setDecimals(0)
            self.gain.setDecimals(2)
            self.gain.setSingleStep(.25)
            if changed:
                self.exposure.setValue(128)
                self.gain.setValue(1)
        if self.worker is None and not legacy:
            for spin in (self.exposure,self.gain):
                spin.setRange(-100000,100000)
                spin.setDecimals(3)
            if simulated and changed:
                self.exposure.setValue(10)
                self.gain.setValue(1)
        self.api.setEnabled(not legacy and self.worker is None)
        self.index.setEnabled(not legacy and self.worker is None)
        self.color_mode.setEnabled(legacy)
        self.gamma.setEnabled(legacy)
        for widget in (self.brightness,self.mirror,self.flip,self.tone_button):widget.setEnabled(legacy)
        self.white_button.setEnabled(legacy and self.worker is not None and self.worker.isRunning())
        self.white_reset_button.setEnabled(self.white_button.isEnabled())
        self.full_save_button.setVisible(legacy)
        self.full_save_button.setEnabled(legacy and self.worker is not None and self.worker.isRunning())
        self.unit_label.setText("Exposure in ms · gain multiplier" if simulated else "Brightness adjusts the preview and saved RGB image. Camera controls are in Advanced." if legacy else "Exposure and gain use OS-specific units.")
        self.auto_button.setEnabled(not simulated and not legacy and self.worker is not None and self.worker.isRunning())
        self.auto_exposure.setEnabled(self.auto_button.isEnabled())
        self.advanced_form.setRowVisible(self.capture_profile,legacy)
        self.advanced_form.setRowVisible(self.api,not legacy)
        self.advanced_form.setRowVisible(self.index,not legacy)
        self.advanced_form.setRowVisible(self.auto_exposure,not legacy and not simulated)
        self.advanced_form.setRowVisible(self.auto_button,not legacy and not simulated)
        self.last_backend=self.backend.currentText()

    def set_connected(self, connected):
        for widget in [self.backend, self.api, self.index, self.capture_profile, self.start_button]:
            widget.setEnabled(not connected)
        for widget in [self.stop_button, self.apply_button, self.save_button]:
            widget.setEnabled(connected)
        self.update_units()

    def start_capture(self):
        if getattr(self, 'setup_process', None) is not None:
            return
        if sys.platform == 'win32' and self.backend.currentText() == 'legacy-tca':
            root = Path(sys.executable).parent if getattr(sys, 'frozen', False) else Path(__file__).resolve().parents[2]
            helper = root / 'support' / 'Camera-USB-Setup.exe'
            if not helper.is_file() or not helper.with_name('libwdi.dll').is_file():
                self.message.setText('Camera USB Setup is missing. Extract the complete portable folder.')
                return
            self.start_button.setEnabled(False)
            self.backend.setEnabled(False)
            self.message.setText('Preparing camera connection. Approve Windows setup if requested…')
            self.setup_process = QProcess(self)
            self.setup_process.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
            self.setup_process.readyReadStandardOutput.connect(self._setup_output)
            self.setup_process.finished.connect(self._setup_finished)
            self.setup_process.errorOccurred.connect(self._setup_failed)
            self.setup_process.start(str(helper), ['--ensure'])
            return
        self._start_capture()

    def _setup_output(self):
        if self.setup_process is not None:
            text = bytes(self.setup_process.readAllStandardOutput()).decode('utf-8', errors='replace').strip()
            if text:
                self.log.appendPlainText(text)

    def _setup_failed(self, error):
        if error == QProcess.ProcessError.FailedToStart:
            self._setup_finished(1, QProcess.ExitStatus.CrashExit)

    def _setup_finished(self, code, status):
        process = self.setup_process
        if process is None:
            return
        self._setup_output()
        self.setup_process = None
        process.deleteLater()
        self.set_connected(False)
        if self.close_pending:
            self.close()
        elif code == 0 and status == QProcess.ExitStatus.NormalExit:
            self._start_capture()
        else:
            self.message.setText('Camera setup failed or was cancelled. See the capture log and USB setup log.')

    def _start_capture(self):
        if self.worker is not None:
            return
        camera=make_camera(self.backend.currentText(), self.index.value(), self.api.currentText())
        if self.backend.currentText()=='legacy-tca' and sys.platform in ('darwin', 'win32'):
            from .process_camera import ProcessCamera
            camera=ProcessCamera()
        if hasattr(camera,'set_capture_profile'):camera.set_capture_profile(self.capture_profile.currentData())
        if hasattr(camera,'configure_controls'):
            camera.configure_controls(self.control_values())
            camera.set_display_mode('color' if self.color_mode.currentText()=='Color' else 'gray')
            camera.set_display_gamma(self.gamma.value())
            camera.set_image_options(self.image_values())
        self.worker = CaptureWorker(camera)
        self.worker.status.connect(self.message.setText)
        self.worker.status.connect(self.log.appendPlainText)
        self.worker.ready.connect(self.capture_ready)
        self.worker.finished.connect(self.finished_capture)
        self.started_at = time.monotonic()
        self.displayed = 0
        self.preview_frames = 0
        self.preview_started_at = None
        self.set_connected(True)
        self.worker.start()

    def capture_ready(self):
        self.message.setText('Connected. Waiting for the first image…')
        self.update_units()

    def stop_capture(self):
        if self.worker:
            self.worker.requestInterruption()
            self.stop_button.setEnabled(False)
            self.save_button.setEnabled(False)
            self.full_save_button.setEnabled(False)
            self.apply_button.setEnabled(False)
            self.auto_button.setEnabled(False)
            self.message.setText("Stopping capture…")

    def finished_capture(self):
        worker = self.worker
        self.worker = None
        if worker:
            worker.deleteLater()
        self.set_connected(False)
        self.stats.setText("Disconnected")
        if self.close_pending:self.close()

    def enqueue(self, command):
        if self.worker and self.worker.isRunning() and not self.worker.isInterruptionRequested():
            try:
                self.worker.submit(command)
            except Exception as exc:
                self.message.setText(f"Unable to queue operation: {exc}")

    def apply_controls(self):
        self.enqueue(("controls", self.control_values()))

    def control_values(self):
        from .color import gain_to_register
        return {"exposure":self.exposure.value(),"gain":gain_to_register(self.gain.value()) if self.backend.currentText()=="legacy-tca" else self.gain.value()}

    def image_values(self):
        return dict(brightness_ev=self.brightness.value(),mirror=self.mirror.isChecked(),flip=self.flip.isChecked(),rotation=180)

    def auto_tone(self):
        import numpy as np, math
        frame=self.worker.snapshot() if self.worker else None
        if frame is not None:
            median=float(np.median(frame.pixels))
            if median>0:self.brightness.setValue(max(-3,min(4,self.brightness.value()+self.gamma.value()*math.log2(128/median))))

    def choose_directory(self):
        directory = QFileDialog.getExistingDirectory(self, "Choose destination", self.directory.text(), options=QFileDialog.Option.DontUseNativeDialog)
        if directory:
            self.directory.setText(directory)

    def refresh(self):
        if not self.worker:
            return
        frame = self.worker.snapshot()
        if frame is None:
            return
        if frame.sequence==self.displayed:return
        if self.displayed==0 and self.message.text().startswith('Connected'):
            self.message.setText('Live. Ready to capture.')
        if frame.controls.get('rejected_hardware_profile'):
            from .color import register_to_gain
            self.exposure.setValue(frame.controls['exposure']['requested'])
            self.gain.setValue(register_to_gain(frame.controls['gain']['requested']))
        self.displayed=frame.sequence
        self.preview_frames+=1
        if self.preview_started_at is None:self.preview_started_at=time.monotonic()
        rgb = frame.pixels
        height, width, _ = rgb.shape
        image = QImage(rgb.data, width, height, rgb.strides[0], QImage.Format.Format_RGB888).copy()
        pixmap = QPixmap.fromImage(image).scaled(self.preview.size(), Qt.AspectRatioMode.KeepAspectRatio,
                                               Qt.TransformationMode.SmoothTransformation)
        self.preview.setPixmap(pixmap)
        elapsed = max(time.monotonic() - self.started_at, 0.001)
        recovery=f" | USB recoveries {frame.controls['usb_recoveries']}" if frame.controls.get('usb_recoveries') else ''
        preview_elapsed=max(time.monotonic()-self.preview_started_at,.001)
        preview_fps=max(self.preview_frames-1,0)/preview_elapsed
        self.stats.setText(f"{width} × {height} | Capture {frame.sequence / elapsed:.1f} fps | Preview {preview_fps:.1f} fps | Frame {frame.sequence}{recovery}")

    def closeEvent(self, event):
        if getattr(self, 'setup_process', None) is not None:
            self.close_pending = True
            self.message.setText('Waiting for camera setup to finish before closing…')
            event.ignore()
            return
        if self.worker and self.worker.isRunning():
            self.close_pending = True
            self.stop_capture()
            self.message.setText("Stopping capture and closing…")
            event.ignore()
        else:
            event.accept()


def main():
    import argparse
    import signal
    parser=argparse.ArgumentParser(description='KohdaLab camera live viewer')
    default_backend = 'legacy-tca' if sys.platform == 'win32' and getattr(sys, 'frozen', False) else 'simulated'
    parser.add_argument('--backend',choices=[b.id for b in available_backends()],default=default_backend)
    parser.add_argument('--camera-mode',choices=['balanced','full'],default='balanced')
    parser.add_argument('--start',action='store_true',help='Connect and start live view immediately')
    args=parser.parse_args()
    app = QApplication.instance() or QApplication([])
    window = CameraWindow()
    window.backend.setCurrentText(args.backend)
    window.capture_profile.setCurrentIndex(window.capture_profile.findData(args.camera_mode))
    window.show()
    for signum in (signal.SIGINT,signal.SIGTERM):
        signal.signal(signum,lambda *_:window.close())
    if args.start:window.start_capture()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
