#! /bin/sh

# Show words that are hard

sqlite3 -column learn.db \
	"select count(8), 'hard words'
	from learn
	where goods > 35
	and interval < 3600
	order by interval"
echo ''
sqlite3 -column learn.db \
	"select word, steno, goods,
	interval / 60,
	(next - strftime('%s')) / 60
	from learn
	where goods > 35
	and interval < 3600
	order by next"
