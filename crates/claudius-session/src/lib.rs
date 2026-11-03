//! What a claudius session does to a song, apart from where it keeps it and how it renders: the edits the API's
//! commands make to a blueprint (`edit`), and the views of the song, its render and its verdict that the API shows
//! (`views`). The server runs them for the studio and the assistants. `host` is a whole session kept in memory, which
//! renders on the spot: the browser's demo runs one in a Web Worker.

#![forbid(unsafe_code)]

pub mod edit;
pub mod host;
pub mod views;
