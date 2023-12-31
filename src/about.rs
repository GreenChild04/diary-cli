use soulog::*;
use crate::{archive::Archive, unwrap_opt};

macro_rules! log_attr {
    ([$entry:ident, $logger:ident] $($name:ident$(($multi:expr))?),* $(,)?) => {$(
        let _multi = true;
        $(let _multi = $multi;)?
        let attr = $entry.$name($logger.hollow());
        if _multi {
            log!(($logger.vital) $name("{attr:#?}\n") as Result);
        } else {
            log!(($logger.vital) $name("{attr:?}") as Result);
        }
    )*}
}

pub fn about(is_moc: bool, uid: String, logger: impl Logger) {
    let archive = Archive::load(logger.hollow());

    if is_moc {
        about_moc(archive, uid, logger)
    } else {
        about_entry(archive, uid, logger)
    }
}

fn about_entry(archive: Archive, uid: String, mut logger: impl Logger) {
    let error_msg = format!("Entry of uid '{uid}' not found in archive");
    let mut entry = unwrap_opt!((archive.get_entry(uid, logger.hollow())) with logger, format: About("{error_msg}"));
    std::mem::drop(error_msg);

    // Print the stuff
    log!((logger) About(""));
    log!((logger.vital) About("{}", colour_format![blue("# "), green("About Entry of uid `"), none(&entry.uid), green("`")]) as Log);
    log_attr! {
        [entry, logger]
        date(false),
        title(false),
        description(false),
        notes,
        tags,
    }
}

fn about_moc(archive: Archive, uid: String, mut logger: impl Logger) {
    let error_msg = format!("MOC of uid '{uid}' not found in archive");
    let mut moc = unwrap_opt!((archive.get_moc(uid, logger.hollow())) with logger, format: About("{error_msg}"));
    std::mem::drop(error_msg);

    // Print the stuff
    log!((logger) About(""));
    log!((logger.vital) About("{}", colour_format![blue("# "), green("About MOC of uid `"), none(&moc.uid), green("`")]) as Log);
    log_attr! {
        [moc, logger]
        tags(false),
        title(false),
        description(false),
        notes,
    }
}