//! The packages and the studio agree on what a project file is: the Mac app's document type, the Windows installer's
//! extension and the Linux desktop's MIME type each name `.claudius`, the extension the studio saves and opens
//! (`claudius_api::PROJECT_EXTENSION`), and none of them makes it a kind of zip, which Safari and Archive Utility
//! would unpack into a folder.

use std::path::Path;

const MIME: &str = "application/x-claudius-project";

fn packaging(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// what `<key>{key}</key>` holds in an Info.plist, up to the end of its value
fn plist_value<'a>(plist: &'a str, key: &str, end: &str) -> &'a str {
    let at = plist.find(&format!("<key>{key}</key>")).unwrap_or_else(|| panic!("no {key}"));
    let rest = &plist[at..];
    &rest[..rest.find(end).unwrap_or_else(|| panic!("{key} never ends")) + end.len()]
}

#[test]
fn the_mac_app_is_the_owner_of_projects_which_are_data_and_not_zips() {
    let plist = packaging("macos/Info.plist");
    let tags = plist_value(&plist, "UTTypeTagSpecification", "</dict>");
    assert!(tags.contains(&format!("<string>{}</string>", claudius_api::PROJECT_EXTENSION)), "{tags}");
    assert!(tags.contains(&format!("<string>{MIME}</string>")), "{tags}");
    let conforms = plist_value(&plist, "UTTypeConformsTo", "</array>");
    assert!(conforms.contains("<string>public.data</string>") && !conforms.contains("zip"), "{conforms}");
    let opens = plist_value(&plist, "CFBundleDocumentTypes", "</array>");
    assert!(opens.contains("<string>io.github.devlato.claudius.project</string>"), "{opens}");
    assert!(
        plist_value(&plist, "UTTypeIdentifier", "</string>").ends_with("io.github.devlato.claudius.project</string>")
    );
}

#[test]
fn windows_and_linux_open_projects_with_the_studio() {
    let ext = claudius_api::PROJECT_EXTENSION;
    let wxs = packaging("windows/claudius.wxs");
    assert!(wxs.contains(&format!(r#"<Extension Id="{ext}" ContentType="{MIME}">"#)));
    assert!(wxs.contains(r#"<Verb Id="open" TargetFile="StudioProgram" Argument="&quot;%1&quot;" />"#));
    let desktop = packaging("linux/claudius-studio.desktop");
    assert!(desktop.lines().any(|l| l == "Exec=claudius studio %f"), "{desktop}");
    assert!(desktop.lines().any(|l| l == format!("MimeType={MIME};")), "{desktop}");
    let types = packaging("linux/claudius-mime.xml");
    assert!(types.contains(&format!(r#"<mime-type type="{MIME}">"#)), "{types}");
    assert!(types.contains(&format!(r#"<glob pattern="*.{ext}"/>"#)), "{types}");
    assert!(!types.contains("sub-class-of"), "a project is no kind of zip here either");
}
