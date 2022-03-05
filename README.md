# Steno Drilling

[Typey-Type](https://didoesdigital.com/typey-type/) is a wonderful
tool for learning steno.  It has a lot of things going for it,
probably most importantly being that it has a set of lesson plans and
drills around learning Plover, or some variant of it.

I began using Typey Type as my primary tool for learning steno, but
after a while decided upon some things I wanted to improve upon:

  - I wanted to learn a _canonical_ stroke for any given word.
    Because Typey Type is reading the text after Plover translates it,
    it can't tell whether you've written the word the way it thinks
    you wanted to.

  - I want to learn things that don't translate into typed text.  For
    example, I'd like to be drilled on variants of punctuation that
    only differ in surrounding spaces.  Or, maybe on strokes to
    send control keys, or command Plover.  Since these aren't typing
    text, Typey Type doesn't see them, and can't really drill for
    them.

  - I want to follow
    [Spaced Reptition](https://en.wikipedia.org/wiki/Spaced_repetition)
    a little more closely.  Typey Type only stores a count of times a
    word is written correctly.  Importantly, it doesn't adjust very
    much when a word is written erroneously.  With true SRS, making a
    mistake will reset that word back to needing to be learned,
    reinforcing it right away, and hopefully unlearning the incorrect
    stroke better.

I ended up writing my own program that I use.  There are a couple of
caveats, however:

  - It is not a web app.  Frankly, this is mostly because I am not a
    web developer.  Writing a local app gives me a bit more
    flexibility as far as data storage.  For example, I can store not
    only a correctly written count, but also a time interval and a
    next time.

  - It is a console app.  It should be fairly portable, given the
    libraries I've used, but it still runs in a terminal window, and
    presents a textual interface, in all its glory.

  - It takes some trickery to get Plover to give it the raw steno
    strokes.  Fortunately, these settings aren't too difficult to
    change.

## Building

Sdrill is a [Rust](https://www.rust-lang.org/) application.  I won't
go into how to install rust, and how to build console applications.
Some day, maybe this will be distributed better.

Once built, it needs lessons to learn.  I used as a starting point,
the Typey Type lesson in its
[github](https://github.com/didoesdigital/typey-type-data) repo.  I
have made some modifications to this, which I will publish in my own
fork of the repo. Namely, since I'm not trying to match translated
text, I can annotate some of the symbols with text to disambiguate
(for example '"' (open) and '"' (close) instead of just '"'.  I also
expand the finger spellings so that it is clear what needs to be
written.

## Setting things up

To begin, you should initialize a learning database.  I will call this
file `learn.db` throughout this document.

```sh
cargo run -- init --db learn.db
```

will initialize a database.  This command will error if the database
has already been initialized.

## Importing lessons

The lessons are expected to be in the format that the various
lesson.txt files are in.  Maybe I will change this to use json
eventually, but I found the text files a little easier to fix up.

It is simple to import one of these files.  For example:

```sh
cargo run -- import --db learn.db \
    ../typey-type-data/fundamentals/introduction/lesson.txt
```

will import the introduction lesson.  Each time a lesson is imported,
those words will be added to the database.

## Seeing progress

At any time, you can view the progress by running the info command:

```sh
cargo run -- info --db learn.db
```

if you have imported many dictionaries that haven't started learning
from, it may be useful to add `--seen` to this command, which will
only show those lessons where at least one word has been learned.

## Learning

### Setting up Plover

Before running sdrill's learn command, configure plover as follows:

  - disable at least the `main.json` dictionary.  If you want to learn
    the commands or things from your user dictionary, you'll have to
    disable those as well.

  - Configure plover to send space after words.

Plover should then just spit out the strokes directly followed by a
space.  The `*` will send enough backspaces to delete the previous
stroke.  Sdrill expects this behavior and should work as long as
plover is only sending strokes.

### Learning

In order to learn, you can simply run the learn command.  You'll need
to provide this command with the number of the lessons you wish to
learn new words from.  If you don't specify a list, or if everything
from that list is being learned, sdrill will merely exit when there
are no pending words to review.

```sh
cargo run -- learn --db learn.db --new 5 --tui
```

The '5' above is the lesson in the lesson list shown by the `info`
command.  The `--tui` command runs the textual UI version.  Without
this option, a very clunky initial version will run.  A future version
of sdrill will make the tui version the default, and likely eliminate
the clunky version.

To learn, begin writing the words shown in the Exercise window.  If
they are correct, the text will slide over to the next word to write.
If you make a mistake, it will be highlighted, and a hint shown of how
to write the word correctly.  In addition, the right side of the
window will show a steno tape that can be helpful in seeing what you
wrote, and possible to correct it.  This is also useful for debugging
if something is wrong with how Plover is sending strokes.

Sdrill will prioritize learning words that are due over learning new
words.

You can stop learning at any time by pressing "Escape" on the
keyboard (or stroking something that translates to that, but that will
probably have to be in a special dictionary, since you turned off the
main Plover dictionaries).

## Re-importing lessons.

The progress of learning is kept separately from the lessons
themselves.  If you make changes to the lessons, it is easy to clear
and re-import the lessons.  You'll need the sqlite3 command line
utility to do this:

```sh
sqlite3 learn.db "DELETE FROM lesson"
sqlite3 learn.db "DELETE FROM list"
```

And then you can run the import commands.  I have placed the sqlite3
commands followed by my desired import commands into a script that I
can rerun to re-import the lessons.

## Suggestions

I recommend that each time you run 'learn', you work your way through
all of the review words, and then learn 5-10 new words.  If you feel
like there are too many reviews, back off on learning new words.

# How this works

The workings of sdrill are fairly simple, but I've found it to be
quite effective, and especially efficient with my time, focusing
review on those words that I need the work on.

All of the data is stored in a single sqlite3 database.  The two
tables `list` and `lesson` hold the imported lessons.  Significant to
this is the 'word' and 'steno' fields of the 'lesson' table.  'word'
is the text that will be displayed to the user, and 'steno' is the raw
steno.  The steno should be written with spaces separating words, and
slashes separating the strokes within a word.  It is picky about the
spacing, although it currently doesn't distinguish between word and
stroke boundaries.  When writing, you will have to write out an entire
entry for it to be accepted.

As words are practiced, the `learn` table is then updated.  `interval`
and `next` implement the SRS algorithm, with the interval increasing
every time the word is written correctly, and being reset back
to the initial value (currently 5 seconds) whenever a mistake is made.
The next value is used to track when a word becomes due, meaning it
has been sufficiently long and needs to be reviewed again.
