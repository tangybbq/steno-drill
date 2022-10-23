#! /bin/sh

# Update the database.

sqlite3 learn.db .dump > S.sql
vi S.sql
mv learn.db learn.bak
sqlite3 learn.db < S.sql
sqlite3 learn.db 'pragma journal_mode=wal'
