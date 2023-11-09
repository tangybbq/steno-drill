#! /bin/bash

# Lapwing drill list import.

sqlite3 learn.db <<ZZZ
DELETE FROM lesson;
DELETE FROM list;
ZZZ

# The lapwing entries are are lists of strokes a tab and a translation.
lap() {
    echo $1 > tmpfile
    echo '' >> tmpfile
    sed s'/^\(.*\)\t\(.*\)/'"'"'\1'"'"': \2/' \
        < lapwing/$1.txt \
        >> tmpfile

    cargo r -- import --db learn.db tmpfile
    rm tmpfile
}

# Make the briefs first, especially to make them easier to eliminate from other definitions.
lap 16-test

lap 5-cvc
lap 5-single-syllable-basic-left-hand
lap 5-single-syllable-EU
lap 5-single-syllable-dbl
lap 5-test

lap 6-single-syllable-fqm
lap 6-single-syllable-gny
lap 6-single-syllable-zvj
lap 6-test

lap 7-single-syllable-oe-ou-and-oeu
lap 7-single-syllable-aeu
lap 7-single-syllable-aou
lap 7-test

lap 8-single-syllable-aoe
lap 8-single-syllable-aoeu
lap 8-single-syllable-au
lap 8-single-syllable-ae
lap 8-single-syllable-ao
lap 8-test

lap 9-single-syllable-rh-v
lap 9-single-syllable-rh-st
lap 9-single-syllable-rh-m-and-k
lap 9-single-syllable-rh-mp-th-and-lk
lap 9-test

lap 10-single-syllable-rh-n-j-lj
# lap 10-...
lap 10-single-syllable-rh-ch-sh-rch
lap 10-test

lap 11-rh-shun-kshun-and-x
lap 11-rh-ment-and-let
lap 11-rh-bl
lap 11-test

lap 12-left-hand-single-syllable-orthographic-chords
