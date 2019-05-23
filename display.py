#!/usr/bin/env python3

from PyQt5 import Qt
import serial

def update():
    res = port.read_until('z')
    if res:
        label.setText('%d rpm' % (float(res.strip().decode()[:-2].strip()) * 60))

port = serial.Serial('/dev/ttyACM0', baudrate=115200, timeout=0.1)
port.flushInput()

app = Qt.QApplication([])
window = Qt.QMainWindow(windowTitle='Frequency Counter')

font = Qt.QFont()
font.setPixelSize(64)
font.setFamily('Monospace')

label = Qt.QLabel(window, font=font, alignment=Qt.Qt.AlignRight)

timer = Qt.QTimer(window, interval=100)
timer.timeout.connect(update)
timer.start()

pal = label.palette()
pal.setColor(Qt.QPalette.Window, Qt.QColor('black'))
pal.setColor(Qt.QPalette.WindowText, Qt.QColor('#55ff55'))
window.setPalette(pal)
window.setCentralWidget(label)
window.setContentsMargins(10, 10, 10, 10)
window.resize(64*10, 80)
window.show()

app.exec_()
