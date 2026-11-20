"""Roda do mouse em rajada sobre a janela do arbor (XTEST). uso: scroll.py <passos> <intervalo_ms>"""
import sys, time
from Xlib import X, display
from Xlib.ext import xtest
d = display.Display()
root = d.screen().root
root.warp_pointer(700, 400)
d.sync()
for _ in range(int(sys.argv[1])):
    xtest.fake_input(d, X.ButtonPress, 5)
    xtest.fake_input(d, X.ButtonRelease, 5)
    d.sync()
    time.sleep(int(sys.argv[2]) / 1000)
