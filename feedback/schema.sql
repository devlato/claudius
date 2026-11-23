-- People's feedback on claudius, one message a row: the day it came, what they wrote, where to answer them (when they
-- gave it), the claudius and system it came from, where they sent it from (studio, demo, site, assistant), and the
-- genre of the song open (when they let it go with the message). Nothing else: no address, no identifier.
create table if not exists feedback (
  id integer primary key autoincrement,
  day text not null,
  text text not null,
  contact text,
  version text,
  os text,
  place text not null,
  song text
);
