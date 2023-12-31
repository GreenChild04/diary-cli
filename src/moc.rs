pub mod collection;

pub use collection::*;
use soulog::*;
use lazy_db::*;
use crate::{entry::*, search::Searchable};
use toml::Table;

// Some ease of life macros
macro_rules! get {
    ($key:ident at $moc:ident from $table:ident as $func:ident with $logger:ident) => {{
        let key = stringify!($key);
        let obj = unwrap_opt!(($table.get(key)) with $logger, format: MOC("moc '{0}' must have '{key}' attribute", $moc));

        unwrap_opt!((obj.$func()) with $logger, format: MOC("moc '{0}'s '{key}' attribute must be of correct type", $moc))
    }};

    ($var:ident = $key:ident at $entry:ident from $table:ident as $func:ident with $logger:ident or $default:expr) => {
        let key = stringify!($key);
        let default = $default;
        let $var = $table.get(key)
            .map(|x| unwrap_opt!((x.$func()) with $logger, format: MOC("moc '{0}'s '{key}' attribute must be of the correct type", $entry)))
            .unwrap_or(&default);
    };
}

pub struct MOC {
    pub container: LazyContainer,
    pub uid: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub notes: Option<Box<[String]>>,
    pub tags: Option<Box<[String]>>,
    pub collections: Option<Box<[Collection]>>,
}

impl MOC {
    pub fn store_lazy(&self, mut logger: impl Logger) {
        log!((logger) MOC("Storing moc into archive..."));
        // Only store them if modified
        if let Some(x) = &self.title { write_db_container!(MOC(self.container) title = new_string(x) with logger); }
        if let Some(x) = &self.description { write_db_container!(MOC(self.container) description = new_string(x) with logger); }
        
        // The bloody lists & arrays
        if let Some(x) = &self.notes {
            list::write(
                x.as_ref(),
                |file, data| LazyData::new_string(file, data),
                &if_err!((logger) [MOC, err => ("While writing notes to archive: {:?}", err)] retry self.container.new_container("notes")),
                logger.hollow()
            );
        }

        if let Some(x) = &self.tags {
            list::write(
                x.as_ref(),
                |file, data| LazyData::new_string(file, data),
                &if_err!((logger) [MOC, err => ("While writing tags to archive: {:?}", err)] retry self.container.new_container("tags")),
                logger.hollow()
            );
        }
    }

    pub fn load_lazy(uid: String, container: LazyContainer) -> Self {
        Self {
            container,
            uid,
            title: None,
            description: None,
            notes: None,
            tags: None,
            collections: None,
        }
    }

    pub fn clear_cache(&mut self) {
        self.title = None;
        self.description = None;
        self.notes = None;
        self.tags = None;
        self.collections = None;
    }

    cache_field!(title(this, logger) -> String {
        read_db_container!(title from MOCSection(this.container) as collect_string with logger)
    });

    cache_field!(description(this, logger) -> String {
        read_db_container!(description from MOC(this.container) as collect_string with logger)
    });

    cache_field!(notes(this, logger) -> Box<[String]> {
        list::read(
            |data| data.collect_string(),
            &if_err!((logger) [MOC, err => ("While reading from moc's notes: {err:?}")] retry this.container.child_container("notes")),
            logger
        )
    });

    cache_field!(tags(this, logger) -> Box<[String]> {
        list::read(
            |data| data.collect_string(),
            &if_err!((logger) [MOC, err => ("While reading from moc's tags: {err:?}")] retry this.container.child_container("tags")),
            logger
        )
    });

    cache_field!(collections(this, logger) -> Box<[Collection]> {
        let container = if_err!((logger) [MOC, err => ("While reading from moc's collections: {err:?}")] retry this.container.child_container("collections"));
        let length = if_err!((logger) [MOC, err => ("While reading from moc's collections' length: {err:?}")] retry container.read_data("length"));
        let length = if_err!((logger) [MOC, err => ("While reading from moc's collections' length: {err:?}")] {length.collect_u16()} crash {
            log!((logger) MOC("{err:#?}") as Fatal);
            logger.crash()
        });
        let mut colletions = Vec::with_capacity(length as usize);

        for i in 0..length {
            colletions.push(Collection::load_lazy(
                if_err!((logger) [MOC, err => ("While reading moc collection {i}: {err:?}")] retry container.child_container(i.to_string()))
            ));
        }

        colletions.into_boxed_slice()
    });

    pub fn new(table: Table, moc_path: &str, database: LazyContainer, mut logger: impl Logger) -> Self {
        log!((logger) MOC("Reading moc '{moc_path}'s raw unchecked data..."));

        let moc_table = get!(moc at moc_path from table as as_table with logger);
        let uid = get!(uid at moc_path from moc_table as as_str with logger).to_string();

        let title = get!(title at moc_path from moc_table as as_str with logger).to_string();
        let description = get!(description at moc_path from moc_table as as_str with logger).to_string();
        get!(raw_notes = notes at moc_path from moc_table as as_array with logger or Vec::<toml::Value>::with_capacity(0));
        let raw_tags = get!(tags at moc_path from moc_table as as_array with logger);
        let raw_collections = get!(collection at moc_path from table as as_array with logger);

        // set the container
        let container =
            if_err!((logger) [MOC, err => ("While initialising moc: '{err:?}'")] retry database.new_container(&uid));

        // parse simple arrays
        log!((logger) MOC("Parsing notes & tags"));
        unpack_array!(notes from raw_notes with logger by x
            => unwrap_opt!((x.as_str()) with logger, format: MOC("All notes in moc '{moc_path}' must be strings")).to_string()
        );

        unpack_array!(tags from raw_tags with logger by x
            => unwrap_opt!((x.as_str()) with logger, format: MOC("All tags in moc '{moc_path}' must be strings")).to_string()
        );

        // parse collections
        log!((logger) MOC("Parsing moc's collections..."));
        let list = if_err!((logger) [MOC, err => ("While initialising collections: {err:?}")] retry container.new_container("collections"));
        unpack_array!(collections from raw_collections with logger by (i, x) => {
            let container = if_err!((logger) [MOC, err => ("While initialising collection {i}: {err:?}")] retry list.new_container(i.to_string()));
            let table = unwrap_opt!((x.as_table()) with logger, format: MOC("MOC '{moc_path}', collection {i} must be a toml table"));
            Collection::new(table, container, moc_path, i as u8, logger.hollow()) // Write into that container
        });
        if_err!((logger) [MOC, err => ("While writing collection list length: {err:?}")] retry write_container!((list) length = new_u16(raw_collections.len() as u16)));

        log!((logger) Entry("Storing moc's parsed and checked data into archive..."));

        let mut this = Self {
            uid,
            container,
            title: Some(title),
            description: Some(description),
            notes: Some(notes.into_boxed_slice()),
            tags: Some(tags.into_boxed_slice()),
            collections: Some(collections.into_boxed_slice()),
        };
        this.store_lazy(logger.hollow());
        log!((logger) MOC("Successfully written moc into archive"));
        log!((logger) MOC(""));
        this.clear_cache();
        this
    }

    pub fn pull(&mut self, logger: impl Logger) -> Table {
        let mut map = Table::new();
        let mut moc = Table::new();

        // Insert uid, title, description, notes, tags, and date
        moc.insert("uid".into(), self.uid.clone().into());
        moc.insert("title".into(), self.title(logger.hollow()).clone().into());
        moc.insert("description".into(), self.description(logger.hollow()).clone().into());
        moc.insert("notes".into(), self.notes(logger.hollow()).to_vec().into());
        moc.insert("tags".into(), self.tags(logger.hollow()).to_vec().into());
        map.insert("moc".into(), moc.into());
        map.insert("is-moc".into(), true.into());

        self.clear_cache();

        map.insert("collection".into(), self.collections(logger.hollow())
            .iter_mut()
            .map(|x| x.pull(logger.hollow()))
            .collect::<Vec<Table>>()
            .into()
        );

        self.clear_cache();

        map
    }
}

impl Searchable for MOC {
    fn get_uid(&self) -> String {
        self.uid.clone()
    }

    fn contains_tag(&mut self, tag: &String, logger: impl Logger) -> bool {
        let result = self.tags(logger).contains(tag);
        self.tags = None;
        result
    }
}