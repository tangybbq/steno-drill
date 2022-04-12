#! /usr/bin/env python

import sqlite3

# Query information about what pending learning we have to do.  I find
# this helpful to know how close I am to whether I should try adding
# some new words.
#
# I usually use a metric for my self along the lines of looking at
# how many words will be ready within the next 10 minutes (nx min),
# and of those, what do the intervals look like.  If the intervals are
# short, it means these are new words (or errors that have to be
# reenforced). Once the intervals for the 10 minutes start to get low,
# I'll then consider adding some new words.

def negfmt(num):
    if num < 0:
        return f"({-num:9.2f})"
    else:
        return f" {num:9.2f} "

# Show words that are hard
def main():
    con = sqlite3.connect('learn.db')
    cur = con.cursor()
    rows = []
    for row in cur.execute("""
            select
            word, goods,
            interval, next - strftime('%s')
            from learn
            order by next
            limit 50"""):
        rows.append(row)
    longest = max([len(x[0]) for x in rows])
    count = 1
    print(f"    {'word':<{longest}}  gd {'iv min ':>11} {'nx min ':>11} {'nx sec ':>11}")
    print(f"    {'':-<{longest}} --- ----------- ----------- -----------")
    for row in rows:
        imin = row[2] / 60
        nmin = row[3] / 60
        print(f"{count:3d} {row[0]:{longest}} {row[1]:>3} {negfmt(imin)} {negfmt(nmin)} {negfmt(row[3])}")
        count += 1

if __name__ == '__main__':
    main()
