//! What a Mac asks of the Claudius app that its window library, winit, does not pass on: the files to open, when a
//! project is double-clicked in the Finder, dropped on the app's icon in the Dock, or opened with Open With.
//!
//! The system sends an app the files to open as an Apple Event, "open documents". AppKit answers it by calling the
//! application's delegate, which winit sets to one of its own that takes no files, and which cannot be replaced
//! without stopping winit's event loop from starting. So a handler of this crate's own answers that one event in
//! place of AppKit's, put in place when the app is about to finish launching
//! (`NSApplicationWillFinishLaunchingNotification`): after AppKit has put its own handlers in place, and before the
//! files the app was opened with arrive, which is when Apple's guide to Apple Events has a handler that replaces one
//! of AppKit's put in place.
//!
//! It is the one crate of claudius with `unsafe` code, since every call into the Objective-C runtime is unsafe in
//! Rust's terms; each block says why it holds. Elsewhere than on a Mac the crate is empty: Windows and Linux start
//! the program with the file to open as its argument (`claudius studio FILE`).

#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

use std::path::PathBuf;

/// Starts taking the files the system asks the app to open: each time it asks, `open` gets them, on the main thread,
/// the files the app was opened with as well as those opened while it runs. Call it once, on the main thread, before
/// the event loop runs (before `eframe::run_native`). Called on another thread it does nothing, as it does on
/// systems other than macOS.
pub fn on_open_files(open: impl Fn(Vec<PathBuf>) + 'static) {
    #[cfg(target_os = "macos")]
    mac::listen(Box::new(open));
    #[cfg(not(target_os = "macos"))]
    let _ = open;
}

#[cfg(target_os = "macos")]
mod mac {
    use std::path::PathBuf;

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
    use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter};

    /// `kCoreEventClass`, "aevt": the events every app answers
    pub(crate) const CORE_EVENTS: u32 = u32::from_be_bytes(*b"aevt");
    /// `kAEOpenDocuments`, "odoc": open these files
    pub(crate) const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
    /// `keyDirectObject`, "----": what an event acts on, here the files
    pub(crate) const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

    pub(crate) struct Ivars {
        open: Box<dyn Fn(Vec<PathBuf>)>,
    }

    define_class!(
        // SAFETY: NSObject asks nothing of a subclass, and `Listener` does not implement `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "ClaudiusOpenFiles"]
        #[ivars = Ivars]
        pub(crate) struct Listener;

        unsafe impl NSObjectProtocol for Listener {}

        impl Listener {
            /// AppKit's handlers are in place: this object answers "open documents" in place of AppKit's.
            // SAFETY: a notification's observer is sent `- (void)name:(NSNotification *)notification`.
            #[unsafe(method(willFinishLaunching:))]
            fn will_finish_launching(&self, _notification: &NSNotification) {
                let events = NSAppleEventManager::sharedAppleEventManager();
                // SAFETY: the manager sends its handler `- (void)name:(NSAppleEventDescriptor *)event
                // withReplyEvent:(NSAppleEventDescriptor *)reply`, which this object answers (below), and holds it
                // without keeping it: it lives as long as the program (`listen`). An event's class and its ID are
                // four-character codes, 32-bit unsigned integers.
                unsafe {
                    let _: () = msg_send![
                        &*events,
                        setEventHandler: self,
                        andSelector: sel!(openDocuments:withReplyEvent:),
                        forEventClass: CORE_EVENTS,
                        andEventID: OPEN_DOCUMENTS
                    ];
                }
            }

            // SAFETY: the signature an Apple Event's handler is sent (above).
            #[unsafe(method(openDocuments:withReplyEvent:))]
            fn open_documents(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
                let files = files(event);
                if !files.is_empty() {
                    (self.ivars().open)(files);
                }
            }
        }
    );

    /// Puts the listener in place (`super::on_open_files`).
    pub(crate) fn listen(open: Box<dyn Fn(Vec<PathBuf>)>) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let this = Listener::alloc(mtm).set_ivars(Ivars { open });
        // SAFETY: NSObject's `init`, on an object of a subclass just allocated, with its own fields set.
        let listener: Retained<Listener> = unsafe { msg_send![super(this), init] };
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: the observer answers the selector with the signature an observer is sent, and lives as long as the
        // program (below); the name is AppKit's own constant.
        unsafe {
            center.addObserver_selector_name_object(
                &listener,
                sel!(willFinishLaunching:),
                Some(NSApplicationWillFinishLaunchingNotification),
                None,
            );
        }
        // the notification centre and the event manager hold it without keeping it: it lives as long as the program
        std::mem::forget(listener);
    }

    /// The files an "open documents" event names: its direct object, a list of them or one alone, each read as a
    /// file's URL, whatever form it came in (an alias, a bookmark or a URL).
    pub(crate) fn files(event: &NSAppleEventDescriptor) -> Vec<PathBuf> {
        // SAFETY: `- (NSAppleEventDescriptor *)paramDescriptorForKeyword:(AEKeyword)keyword`, a keyword being a
        // four-character code, a 32-bit unsigned integer; nil when the event has no such parameter.
        let named: Option<Retained<NSAppleEventDescriptor>> =
            unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
        let Some(named) = named else { return Vec::new() };
        // a list's items count from 1
        let items: Vec<Retained<NSAppleEventDescriptor>> = match named.numberOfItems() {
            n if n > 0 => (1..=n).filter_map(|i| named.descriptorAtIndex(i)).collect(),
            _ => vec![named],
        };
        items.iter().filter_map(|item| item.fileURLValue()?.to_file_path()).collect()
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::path::PathBuf;

    use objc2::rc::Retained;
    use objc2::{ClassType, msg_send};
    use objc2_foundation::{NSAppleEventDescriptor, NSURL};

    use super::mac::{CORE_EVENTS, DIRECT_OBJECT, OPEN_DOCUMENTS, files};

    /// "open documents" as the Finder sends it, naming `paths`: in a list, or one alone
    fn opening(paths: &[&str], listed: bool) -> Retained<NSAppleEventDescriptor> {
        // SAFETY: `+ appleEventWithEventClass:(AEEventClass)eventClass eventID:(AEEventID)eventID
        // targetDescriptor:(NSAppleEventDescriptor *)target returnID:(AEReturnID)returnID
        // transactionID:(AETransactionID)transactionID`: two four-character codes, a target that may be nil, and a
        // 16-bit and a 32-bit signed integer (kAutoGenerateReturnID, kAnyTransactionID).
        let event: Retained<NSAppleEventDescriptor> = unsafe {
            msg_send![
                NSAppleEventDescriptor::class(),
                appleEventWithEventClass: CORE_EVENTS,
                eventID: OPEN_DOCUMENTS,
                targetDescriptor: None::<&NSAppleEventDescriptor>,
                returnID: -1i16,
                transactionID: 0i32
            ]
        };
        let file = |p: &str| NSAppleEventDescriptor::descriptorWithFileURL(&NSURL::from_file_path(p).expect("a path"));
        let named = if listed {
            let list = NSAppleEventDescriptor::listDescriptor();
            // index 0 puts each at the list's end
            paths.iter().for_each(|p| list.insertDescriptor_atIndex(&file(p), 0));
            list
        } else {
            file(paths[0])
        };
        // SAFETY: `- (void)setParamDescriptor:(NSAppleEventDescriptor *)descriptor forKeyword:(AEKeyword)keyword`.
        let _: () = unsafe { msg_send![&*event, setParamDescriptor: &*named, forKeyword: DIRECT_OBJECT] };
        event
    }

    #[test]
    fn opening_files_names_each_as_the_finder_sends_them() {
        let paths = ["/Users/someone/Music/Glass Garden.claudius", "/Volumes/Drive/old songs/remix.claudius"];
        let all: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        assert_eq!(files(&opening(&paths, true)), all);
        assert_eq!(files(&opening(&paths[..1], false)), all[..1]);
    }

    #[test]
    fn an_event_that_names_no_files_opens_none() {
        // SAFETY: as in `opening`.
        let event: Retained<NSAppleEventDescriptor> = unsafe {
            msg_send![
                NSAppleEventDescriptor::class(),
                appleEventWithEventClass: CORE_EVENTS,
                eventID: OPEN_DOCUMENTS,
                targetDescriptor: None::<&NSAppleEventDescriptor>,
                returnID: -1i16,
                transactionID: 0i32
            ]
        };
        assert!(files(&event).is_empty());
    }
}
