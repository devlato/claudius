//! What a Mac asks of the Claudius app that its window library, winit, does not pass on: the files to open, when a
//! project is double-clicked in the Finder, dropped on the app's icon in the Dock, or opened with Open With; and where
//! the window's own buttons go, on the line of the studio's top bar, which is the window's title bar.
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

/// Keeps each window's close, minimize and zoom buttons on the line of a top bar that runs under its hidden title bar:
/// their middles `middle` points below the window's top, the close button's left edge `left` points from the window's,
/// the other two after it as AppKit spaces them. AppKit puts them back where a title bar of its own has them each time
/// it lays a window out (its first showing, a resize, a return from full screen, the window coming to the front or
/// going behind), so they are moved again after each, before the window is drawn; in full screen AppKit keeps them, in
/// the bar that slides down from the menu bar. Call it once, on the main thread, before the event loop runs; elsewhere
/// than on a Mac, or called on another thread, it does nothing.
pub fn window_buttons_at(middle: f64, left: f64) {
    #[cfg(target_os = "macos")]
    mac::buttons_at(middle, left);
    #[cfg(not(target_os = "macos"))]
    let _ = (middle, left);
}

/// Where the window's buttons go, in a title bar's own terms: the height of the strip at the window's top they are
/// centred in, and each one's left edge, from the close button's at `left` on, `step` apart (AppKit's spacing).
pub fn buttons_layout(middle: f64, left: f64, step: f64) -> (f64, [f64; 3]) {
    (2.0 * middle, [left, left + step, left + 2.0 * step])
}

#[cfg(target_os = "macos")]
mod mac {
    use std::path::PathBuf;

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSApplicationWillFinishLaunchingNotification, NSButton, NSWindow, NSWindowButton,
        NSWindowDidBecomeKeyNotification, NSWindowDidChangeOcclusionStateNotification,
        NSWindowDidEndLiveResizeNotification, NSWindowDidExitFullScreenNotification, NSWindowDidResignKeyNotification,
        NSWindowDidResizeNotification, NSWindowDidUpdateNotification, NSWindowStyleMask,
    };
    use objc2_foundation::{
        NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSPoint,
    };

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

    /// where `window_buttons_at` puts the buttons
    pub(crate) struct Place {
        middle: f64,
        left: f64,
    }

    define_class!(
        // SAFETY: NSObject asks nothing of a subclass, and `Keeper` does not implement `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "ClaudiusWindowButtons"]
        #[ivars = Place]
        pub(crate) struct Keeper;

        unsafe impl NSObjectProtocol for Keeper {}

        impl Keeper {
            /// A window was laid out again: its buttons go back on the top bar's line.
            // SAFETY: a notification's observer is sent `- (void)name:(NSNotification *)notification`.
            #[unsafe(method(laidOut:))]
            fn laid_out(&self, notification: &NSNotification) {
                if let Some(window) = notification.object().and_then(|o| o.downcast::<NSWindow>().ok()) {
                    place(&window, self.ivars().middle, self.ivars().left);
                }
            }
        }
    );

    /// Puts the keeper in place, and the buttons of the windows already open (`super::window_buttons_at`).
    pub(crate) fn buttons_at(middle: f64, left: f64) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let this = Keeper::alloc(mtm).set_ivars(Place { middle, left });
        // SAFETY: NSObject's `init`, on an object of a subclass just allocated, with its own fields set.
        let keeper: Retained<Keeper> = unsafe { msg_send![super(this), init] };
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: the observer answers the selector with the signature an observer is sent, and lives as long as the
        // program (below); the names are AppKit's own constants, and a nil object observes every window.
        unsafe {
            // a window shown for the first time changes from hidden to seen, whether or not it becomes the key window,
            // and every window is updated after each event the app handles: placed then too, they are in place before
            // the window's first frame is drawn, and a lay-out AppKit does that no other notice tells of is undone at
            // the next event (`place` does nothing when they are where they belong)
            for name in [
                NSWindowDidUpdateNotification,
                NSWindowDidChangeOcclusionStateNotification,
                NSWindowDidResizeNotification,
                NSWindowDidEndLiveResizeNotification,
                NSWindowDidExitFullScreenNotification,
                NSWindowDidBecomeKeyNotification,
                NSWindowDidResignKeyNotification,
            ] {
                center.addObserver_selector_name_object(&keeper, sel!(laidOut:), Some(name), None);
            }
        }
        let windows = NSApplication::sharedApplication(mtm).windows();
        for i in 0..windows.count() {
            place(&windows.objectAtIndex(i), middle, left);
        }
        // the notification centre holds it without keeping it: it lives as long as the program
        std::mem::forget(keeper);
    }

    /// The window's buttons on the line `middle` points below its top, the close button `left` points in: the strip
    /// of the window that holds them (AppKit's title bar container) made twice `middle` high at the window's top, and
    /// the buttons centred in it, spaced as AppKit spaced them.
    pub(crate) fn place(window: &NSWindow, middle: f64, left: f64) {
        if window.styleMask().contains(NSWindowStyleMask::FullScreen) {
            return;
        }
        let buttons: Vec<Retained<NSButton>> =
            [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton]
                .into_iter()
                .filter_map(|b| window.standardWindowButton(b))
                .collect();
        let Some(close) = buttons.first() else { return };
        // SAFETY: a view's superview, which AppKit gives as a view or nil; a window button's are its title bar's view
        // and that view's container, AppKit's own.
        let Some(strip) = (unsafe { close.superview() }).and_then(|bar| unsafe { bar.superview() }) else { return };
        let size = close.frame().size;
        let step = buttons.get(1).map_or(size.width + 6.0, |next| next.frame().origin.x - close.frame().origin.x);
        let (height, lefts) = super::buttons_layout(middle, left, step);
        let y = (height - size.height) / 2.0;
        let near = |a: f64, b: f64| (a - b).abs() < 0.25;
        let at = close.frame().origin;
        if near(strip.frame().size.height, height) && near(at.x, lefts[0]) && near(at.y, y) {
            return;
        }
        let mut frame = strip.frame();
        frame.size.height = height;
        frame.origin.y = window.frame().size.height - height;
        strip.setFrame(frame);
        for (button, x) in buttons.iter().zip(lefts) {
            button.setFrameOrigin(NSPoint::new(x, y));
        }
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

#[cfg(test)]
mod layout_tests {
    #[test]
    fn the_buttons_are_centred_on_the_bars_line_as_far_apart_as_appkit_spaces_them() {
        // the studio's bar is 58 points high; AppKit spaces its buttons 23 points apart (macOS 27)
        let (strip, lefts) = super::buttons_layout(29.0, 20.0, 23.0);
        assert_eq!(strip, 58.0, "a strip twice the line's depth centres a button of any height on the line");
        assert_eq!(lefts, [20.0, 43.0, 66.0]);
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
