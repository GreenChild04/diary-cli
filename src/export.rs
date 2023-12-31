use std::path::Path;
use crate::{entry::{Entry, Section}, Scribe, scribe_write, archive::Archive, search, moc::{MOC, Collection}, sort::sort_uids};
use soulog::*;

pub fn export_md(strict: bool, tags: Option<Vec<String>>, path: String, mut logger: impl Logger) {
    log!((logger) Export("Exporting archive to path '{path}'..."));
    let archive = Archive::load(logger.hollow());

    // Get entries and mocs
    let mut entries = match &tags {
        Some(x) => 
            (if strict { search::search(x, archive.list_entries(logger.hollow()), logger.hollow()) }
            else { search::search_strict(x, archive.list_entries(logger.hollow()), logger.hollow()) })
                .into_iter().map(|x| archive.get_entry(x, logger.hollow()).unwrap()).collect(),
        None => archive.list_entries(logger.hollow()),
    };
    let mut mocs = match &tags {
        Some(x) => 
            (if strict { search::search(x, archive.list_mocs(logger.hollow()), logger.hollow()) }
            else { search::search_strict(x, archive.list_mocs(logger.hollow()), logger.hollow()) })
                .into_iter().map(|x| archive.get_moc(x, logger.hollow()).unwrap()).collect(),
        None => archive.list_mocs(logger.hollow()),
    };

    // Export em
    let path = Path::new(&path);
    entries.iter_mut().for_each(|x| export_entry(path, x, logger.hollow()));
    mocs.iter_mut().for_each(|x| export_moc(path, x, &archive, logger.hollow()));

    log!((logger.vital) Export("Successfully exported all specified items") as Log);
}

pub fn export_entry(path: &Path, entry: &mut Entry, mut logger: impl Logger) {
    log!((logger) Export("Exporting entry of uid '{}'...", entry.uid));
    let mut scribe = Scribe::new(path.join(&entry.uid).with_extension("md"), logger.hollow());

    // Tags, title and description
    let date = *entry.date(logger.hollow());
    scribe_tags_n_date(entry.tags(logger.hollow()), &date, &mut scribe);
    scribe_write!((scribe) "# ", entry.title(logger.hollow()), "\n");
    scribe.write_line("---");
    scribe_write!((scribe) "**Description:** ", entry.description(logger.hollow()), "\n\n");

    // Notes
    let notes = entry.notes(logger.hollow());
    let mut notes_header_written_to: bool = false;
    if notes.len() > 0 {
        notes_header_written_to = true;
        scribe.write_line("## Notes");
        notes.iter().for_each(|x| scribe_write!((scribe) "- ", x, "\n"));  
    }

    // Sections' notes
    entry.sections(logger.hollow()).iter_mut().for_each(|section| {
        let title = section.title(logger.hollow()).clone();
        let notes = section.notes(logger.hollow());
        if notes.len() > 0 {
            if !notes_header_written_to { scribe.write_line("## Notes"); notes_header_written_to = true; }
            scribe_write!((scribe) "- #### ", &title, "\n");
            notes.iter().for_each(|x| scribe_write!((scribe) "\t- ", x, "\n"));
        } section.clear_cache();
    });
    scribe.write_line("---");

    // Sections
    entry.sections(logger.hollow()).iter_mut().for_each(|x| export_section_content(&mut scribe, x, logger.hollow()));

    entry.clear_cache();
}

pub fn export_moc(path: &Path, moc: &mut MOC, archive: &Archive, mut logger: impl Logger) {
    log!((logger) Export("Exporting moc of uid '{}'...", moc.uid));
    let mut scribe = Scribe::new(path.join(&moc.uid).with_extension("md"), logger.hollow());

    // Tags, title and description
    scribe_tags(moc.tags(logger.hollow()), &mut scribe);
    scribe_write!((scribe) "# ", moc.title(logger.hollow()), "\n");
    scribe.write_line("---");
    scribe_write!((scribe) "**Description:** ", moc.description(logger.hollow()), "\n\n");

    // Notes
    let notes = moc.notes(logger.hollow());
    if notes.len() > 0 {
        scribe.write_line("## Notes");
        notes.iter().for_each(|x| scribe_write!((scribe) "- ", x, "\n"));  
    }

    // Collections' notes
    moc.collections(logger.hollow()).iter_mut().for_each(|collection| {
        let title = collection.title(logger.hollow()).clone();
        let notes = collection.notes(logger.hollow());
        if notes.len() > 0 {
            scribe_write!((scribe) "- #### ", &title, "\n");
            notes.iter().for_each(|x| scribe_write!((scribe) "\t- ", x, "\n"));
        } collection.clear_cache();
    });
    scribe.write_line("---");

    // Collections
    moc.collections(logger.hollow()).iter_mut().for_each(|x| export_collection_content(&mut scribe, x, archive, logger.hollow()));

    moc.clear_cache();
}

fn export_collection_content(scribe: &mut Scribe<impl Logger>, collection: &mut Collection, archive: &Archive, logger: impl Logger) {
    let tags = collection.include(logger.hollow());

    let moc_uids = search::search_strict(tags, archive.list_mocs(logger.hollow()), logger.hollow());
    let mut entry_uids = search::search_strict(tags, archive.list_entries(logger.hollow()), logger.hollow());

    if moc_uids.is_empty() && entry_uids.is_empty() { return; }
    scribe_write!((scribe) "## ", collection.title(logger.hollow()), "\n");

    entry_uids = sort_uids(&entry_uids, logger.hollow()).to_vec(); // Sorting stuff

    moc_uids.into_iter()
        .map(|x| archive.get_moc(x, logger.hollow()).unwrap())
        .enumerate()
        .for_each(|(i, mut entry)| {
            scribe_write!((scribe) &(i + 1).to_string(), ". \\[[", entry.title(logger.hollow()), "](", &entry.uid, ")\\] ", entry.description(logger.hollow()), &format!(" `notes: {:?}`\n", entry.notes(logger.hollow())));
            entry.clear_cache();
        });

    entry_uids.into_iter()
        .map(|x| archive.get_entry(x, logger.hollow()).unwrap())
        .enumerate()
        .for_each(|(i, mut entry)| {
            scribe_write!((scribe) &(i + 1).to_string(), ". \\[[", entry.title(logger.hollow()), "](", &entry.uid, ")\\] ", entry.description(logger.hollow()), &format!(" `notes: {:?}`\n", entry.notes(logger.hollow())));
            entry.clear_cache();
        });
}

fn export_section_content(scribe: &mut Scribe<impl Logger>, section: &mut Section, logger: impl Logger) {
    scribe_write!((scribe) "### ", section.title(logger.hollow()), "\n");
    let content = section.content(logger.hollow()).trim_end_matches('\n').split('\n');
    content.for_each(|x| {
        scribe_write!((scribe) "> ", x, "\n");
    });
    section.clear_cache();
}

fn scribe_tags(tags: &[String], scribe: &mut Scribe<impl Logger>) {
    scribe.write_line("---");
    scribe.write("tags:\n  - obsidian-md\n  - diary-cli\n");
    tags.iter().for_each(|x| scribe_write!((scribe) "  - ", x, "\n"));
    scribe.write_line("---");
}

fn scribe_tags_n_date(tags: &[String], date: &[u16; 3], scribe: &mut Scribe<impl Logger>) {
    scribe.write_line("---");
    scribe.write("tags:\n  - obsidian-md\n  - diary-cli\n");
    tags.iter().for_each(|x| scribe_write!((scribe) "  - ", x, "\n"));
    scribe.write(&format!("date: {0}-{1}-{2}\n", date[2], date[1], date[0]));
    scribe.write_line("---");
}