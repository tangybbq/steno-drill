#! /usr/bin/env python3

import math
import sqlite3
from signal import signal, SIGPIPE, SIG_DFL

# Just ignore sig pipe to avoid errors when piping through things like
# 'head'.
signal(SIGPIPE, SIG_DFL)

# Try to estimate a daily burden.  The idea here is to try and
# estimate how much practice is needed to keep up, or get ahead.
#
# Every new word requires some amount of work in order to learn.
# Assuming a 5 second initial interval, approximately doubling the
# interval when written correctly, and that we can consider a word
# fully learned after a year, that is approximately 25 doublings.
# Assuming the user writes at about 20WPM, that is about 1.25 minutes
# work of cumulative work for each word learned.
#
# However, when a mistake is made, the repetitions up until that point
# will have to be done again.  We will count this, using the "errors"
# table, broken by days, so we can tally up this "burden" introduced
# by having to relearn various words.
#
# For now, consider days to start each day at 4AM, local time.

# Compute time spent each day.
def compute_daily(cur):
    elts = {}
    for row in cur.execute("""
            SELECT
                date(start, '-9 hours'),
                (SUM(julianday(stop) - julianday(start))) * 24 * 60
            FROM
                history
            GROUP BY
                date(start, '-9 hours')
            ORDER BY
                start"""):
        date, time = row
        elts[date] = time
    return elts


class Track():
    def __init__(self, daily):
        self.daily = daily
        self.last_date = None
        self.cost = 0.0

    def ship(self):
        if self.last_date is None:
            return
        total = self.daily.get(self.last_date, 0)
        print("{}     burden:{:6.1f}     total:{:6.1f}     over:{:6.1f}".format(self.last_date, self.cost,
            total, total - self.cost))
        self.last_date = None
        self.cost = 0.0

    def add(self, date, interval):
        if date != self.last_date:
            self.ship()
            self.last_date = date
        self.cost += math.log2(interval / 5.0) * 3.0 / 60.0

def main():
    con = sqlite3.connect('learn.db')
    cur = con.cursor()
    daily = compute_daily(cur)
    track = Track(daily)
    for row in cur.execute("""
            SELECT
                date(stamp, '-9 hours'),
                interval
            FROM
                errors
            ORDER BY
                stamp DESC"""):
        date, interval = row
        track.add(date, interval)
    track.ship()

if __name__ == '__main__':
    main()
