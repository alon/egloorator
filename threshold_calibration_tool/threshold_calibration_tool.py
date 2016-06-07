#!/bin/env python3

"""
Calibrate A microphone simple average and threshold Voice Activity Detector.

User based:

    user sets threshold with slider.
    plot shows which areas of a prerecorded sound are speech (above threshold)
    and which are silent (below threshold)

What level does (from gst-plugins-1.0 - version NNN):
    rms = 20 * log10(sqrt(sum of squares for interval / num_samples)) # 0.1 second default

"""

import os
import sys
import argparse
from datetime import datetime

import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk
gi.require_version('Gst', '1.0')
from gi.repository import Gst

from gi.repository import GLib

# hack
import matplotlib
matplotlib.rcParams['backend'] = 'Gtk3Cairo'

from matplotlib.figure import Figure
from matplotlib.backends.backend_gtk3cairo import FigureCanvas
import matplotlib
matplotlib.use('GTK3Cairo')

import matplotlib.pyplot as plt
import numpy as np
import pylab as pl

import wave
import array as array_mod

import numpy as np
from matplotlib.widgets import Slider, Button, RadioButtons


parser = argparse.ArgumentParser()
parser.add_argument('--filename', default='example.wav')
args = parser.parse_args()
sample_freq = 44100
dt = 1.0 / sample_freq
average_len = 4410 # 100 milliseconds


def save_wav(filename, wave_params, values):
    with wave.open(filename, 'wb') as f:
        f.setframerate(wave_params['frame_rate'])
        f.setnchannels(wave_params['n_channels'])
        f.setsampwidth(wave_params['sample_width'])
        f.writeframes(values) # tobytes()

def load_wav(filename):
    with wave.open(filename) as w:
        frame_rate = w.getframerate()
        frames = w.readframes(w.getnframes())
        sample_width = w.getsampwidth()
        n_channels = w.getnchannels()
        if w.getnframes() > 60 * frame_rate:
            print("taking one minute around max of the range")
            start = np.argmax(frames)
            #start = sample_width * w.getnframes() // 2 # start at middle
            frames = frames[start:start + sample_width * frame_rate * 60]
    d = dict(frame_rate=frame_rate, sample_width=sample_width, n_channels=n_channels)
    return array_mod.array('h', frames), d

samples_pre_normalize, wave_params = load_wav(args.filename)
print("got wave file parameters: {}".format(wave_params))
print("from file max: {}, min: {}".format(max(samples_pre_normalize), min(samples_pre_normalize)))

samples_np_array_pre_normalize = np.array(samples_pre_normalize, dtype='float64')

samples_raw = samples_np_array_pre_normalize / float(2**15)
print("normalized max: {}, min: {}".format(max(samples_raw), min(samples_raw)))
# do what the level gstreamer plugin does first: compute power in db
# so average is weird, i.e. done after log10. gives more value to very low
# sounds - not what we want.
samples_power = (samples_raw * samples_raw)
# cut it up
sum_power = np.array([sum(samples_power[i:i + average_len]) for i in range(0, len(samples_power), average_len)])
averaged = (sum_power / average_len) ** 0.5
samples_db = 20 * np.log10(averaged + 1e-35)
print("samples_db max: {}, min: {}".format(max(samples_db), min(samples_db)))
threshold = (max(samples_db) + min(samples_db)) / 2.0
print("setting threshold to {}".format(threshold))
samples_time = np.linspace(0.0, len(samples_db) / sample_freq * average_len, len(samples_db))
print("time interval [{}, {}]".format(samples_time[0], samples_time[-1]))

#freq =

#spectral_slope = [pl.polyfit(freq,
spectral_slope = samples_db


def calc_optimal_threshold():
    print("TODO")

def play_above_threshold():
    print("TODO")


"""
rax = plt.axes([0.025, 0.5, 0.15, 0.15], axisbg=axcolor)
radio = RadioButtons(rax, ('red', 'blue', 'green'), active=0)

def colorfunc(label):
    l.set_color(label)
    fig.canvas.draw_idle()
radio.on_clicked(colorfunc)
"""


class MyWindow(Gtk.Window):

    def __init__(self, t, feature, reference):
        self.t = t
        self.feature = feature
        Gtk.Window.__init__(self, title="Power Voice Activity Detector threshold calibrator")

        self.main_vbox = Gtk.VBox()
        self.add(self.main_vbox)
        self.button_hbox = button_hbox = Gtk.HBox()
        self.plot_hbox = Gtk.HBox()
        self.main_vbox.add(self.plot_hbox)
        self.main_vbox.add(self.button_hbox)
        def add_button(label, cb):
            button = Gtk.Button(label=label)
            button.connect("clicked", cb)
            self.button_hbox.add(button)
        add_button('Reset', self.on_reset)
        add_button('Auto', self.on_auto)
        add_button('Play Above', self.on_play_above)
        min_adj = min(feature)
        max_adj = max(feature)
        self.threshold = (min_adj + max_adj) / 2
        self.adjustment = Gtk.Adjustment(self.threshold, min_adj, max_adj, (max_adj - min_adj) / 100.0, (max_adj - min_adj) / 10.0, 0)
        self.scale = Gtk.Scale(orientation=Gtk.Orientation.HORIZONTAL,
                               adjustment=self.adjustment)
        self.scale.connect('change-value', self.on_threshold_scale)
        self.button_hbox.add(self.scale)
        self.create_figure()

    def create_time_bars(self):
        min_y, max_y = self.axes_left.get_ylim()
        self.time_bar_line_left, = self.axes_left.plot([self.t[0], self.t[0]], [min_y, max_y], 'r')
        min_y, max_y = self.axes_right.get_ylim()
        self.time_bar_line_right, = self.axes_right.plot([self.t[0], self.t[0]], [min_y, max_y], 'r')
        self.time_bar_lines = [self.time_bar_line_left, self.time_bar_line_right]
        self.update_time_bars((self.t[-1] + self.t[0]) / 2)

    def update_time_bars(self, t):
        for time_bar_line in self.time_bar_lines:
            time_bar_line.set_xdata([t] * 2)
        self.fig.canvas.draw_idle()

    def create_figure(self):
        self.fig = fig = Figure(figsize=(5, 4), dpi=100)
        self.axes_left = a = fig.add_subplot(111)
        canvas = FigureCanvas(fig)
        canvas.set_size_request(800, 600)
        self.plot_hbox.add(canvas)
        #plt.subplots_adjust(left=0.25, bottom=0.25)

        sound_line, = a.plot(self.t, self.feature, color='red')

        # draw threshold line
        self.threshold_line, = a.plot([self.t[0], self.t[-1]], [threshold] * 2)

        # calculate specgram (spectrogram)
        self.spec_fig = Figure(figsize=(5, 4), dpi=100)
        self.axes_right = a_specgram = self.spec_fig.add_subplot(111)
        a_specgram.specgram(samples_raw, NFFT=1024, Fs=sample_freq, noverlap=900)
        canvas_specgram = FigureCanvas(self.spec_fig)
        canvas_specgram.set_size_request(800, 600)
        self.plot_hbox.add(canvas_specgram)

        self.create_time_bars()

    def on_reset(self, widget):
        self.threshold_slider.reset()

    def on_auto(self, widget):
        new_threshold = calc_optimal_threshold()
        self.threshold_slider.set(new_threshold)

    def update_threshold_line(self, val):
        self.threshold_line.set_ydata([val] * 2)
        self.fig.canvas.draw_idle()

    def on_threshold_scale(self, widget, bla, val):
        #print("{}, {}, {}".format(repr(widget), repr(bla), val))
        self.threshold = val
        self.update_threshold_line(val)

    def start_time_bar_play(self, time_indices):
        # [0, 1] [3 4]
        # 0.5 => 0.5
        # 1.5 => 3.5
        # O(|time_indices|**2), can improve later
        start_timeout = datetime.now()

        def get_t_from_wallclock():
            dt_from_start = (datetime.now() - start_timeout).total_seconds()
            total_t = 0.0
            for i, (start, finish) in enumerate(time_indices):
                start, finish = start * dt, finish * dt
                if total_t >= dt_from_start and total_t + finish - start >= dt_from_start:
                    return start
                total_t += finish - start
            return None

        def on_timeout():
            t = get_t_from_wallclock()
            if t is None:
                return False
            self.update_time_bars(t)
            return True

        GLib.timeout_add(50, on_timeout)

    def on_play_above(self, widget):
        segments = []
        for i, val in enumerate(self.feature):
            if val > self.threshold:
                segments.append((i * average_len, (i + 1) * average_len))
        values = sum([samples_pre_normalize[start:finish]
                      for start, finish in segments], array_mod.array('h'))
        filename = 'overwritten_{}_{}.wav'.format(args.filename, self.threshold)
        save_wav(filename=filename,
                 wave_params=wave_params, values=values)
        # Hack: open loop - progress our timebar now, same time as xdg-open, sorta..
        self.start_time_bar_play(segments)
        os.system('xdg-open {} &'.format(filename))


def main():
    win = MyWindow(t=samples_time, feature=spectral_slope, reference=samples_db)
    win.connect("delete-event", Gtk.main_quit)
    win.show_all()
    Gtk.main()


if __name__ == '__main__':
    main()
