use lazy_db::*;
use crate::home_dir;
use crate::list;
use crate::unwrap_opt;
use soulog::*;
use std::fs;
use std::path::PathBuf;
use std::path::Path;
use crate::entry::Entry;
use crate::moc::MOC;

pub struct Archive {
    database: LazyDB,
    uid: u64,
    pub itver: u16,
}

impl Archive {
    /// Initialises a new archive, will throw error if one already exists
    pub fn init(mut logger: impl Logger) -> Self {
        let path = home_dir().join("archive");
        let path_string = path.to_string_lossy();
        // Check if archive already exists
        if path.exists() {
            log!((logger.error) Init("Archive '{path_string}' already exists, try wiping it before initialising again") as Fatal);
            return logger.crash()
        }

        log!((logger) Init("Initialising a new archive at '{path_string}'..."));
        let database = if_err!((logger) [Init, err => ("While initialising database: {err:?}")] retry LazyDB::init(&path));
        
        let uid = {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hasher};
            RandomState::new().build_hasher().finish()
        };
        let itver = 0u16;

        log!((logger) Init("Writing uid and itver to archive..."));
        if_err!((logger) [Init, err => ("While writing uid: {err:?}")] retry write_database!((&database) uid = new_u64(uid)));
        if_err!((logger) [Init, err => ("While writing itver: {err:?}")] retry write_database!((&database) itver = new_u16(itver)));

        log!((logger) Init("Initialising sorted and unsorted entry containers..."));
        if_err!((logger) [Init, err => ("While writing stack length: {err:?}")] retry write_database!((&database) /order/sorted::length = new_u16(0)));
        if_err!((logger) [Init, err => ("While writing stack length: {err:?}")] retry write_database!((&database) /order/unsorted::length = new_u16(0)));

        log!((logger.vital) Init("Successfully initialised archive '{path_string}'") as Log);
        Self {
            database,
            uid,
            itver,
        }
    }

    /// Loads an archive at the cli's home
    #[inline]
    pub fn load(logger: impl Logger) -> Self {
        let path = home_dir().join("archive");
        Self::load_dir(path, logger)
    }

    /// Loads an archive at a specified path
    pub fn load_dir(path: PathBuf, mut logger: impl Logger) -> Self {
        let path_string = path.to_string_lossy();
        log!((logger) Archive("Loading archive '{path_string}'..."));

        // Checks if path exists or not
        if !path.is_dir() {
            log!((logger.vital) Archive("Archive '{path_string}' not found; initialising a new one...") as Inconvenience);
            return Self::init(logger)
        };

        let database = if_err!((logger) [Archive, err => ("While loading archive '{path_string}': {err:?}")] retry LazyDB::load_dir(&path));
        log!((logger) Archive("Loading uid and itver of archive..."));
        let uid = if_err!((logger) [Archive, err => ("While loading archive uid: {err:?}")] retry (|| search_database!((&database) uid)?.collect_u64())());
        let itver = if_err!((logger) [Archive, err => ("While loading archive itver: {err:?}")] retry (|| search_database!((&database) itver)?.collect_u16())());

        log!((logger.verbose) Archive("Successfully loaded archive at '{path_string}'") as Log);
        log!((logger) Archive(""));

        Self {
            database,
            uid,
            itver,
        }
    }

    /// Rolls back to last backup
    pub fn rollback(force: bool, mut logger: impl Logger) {
        log!((logger) RollBack("Rolling back to last backup..."));
        log!((logger.vital) RollBack("Rollback cannot revert successful commits; only unsuccessful ones that corrupt the archive.") as Warning);
        let path = home_dir().join("backup.ldb");
        if !path.is_file() {
            log!((logger.error) RollBack("No recent backups made; cannot rollback") as Fatal);
            return logger.crash();
        } Self::load_backup(path, force, logger.hollow());
        log!((logger.vital) RollBack("Successfully rolled back to last backup") as Log);
    }

    /// Backs up home archive to specified path
    pub fn backup(out_path: impl AsRef<Path>, mut logger: impl Logger) {
        let out_path = out_path.as_ref();
        let path = home_dir().join("archive");
        let path_string = path.to_string_lossy();
        let out_string = out_path.to_string_lossy();
        
        log!((logger) Backup("Backing up archive '{path_string}' as '{out_string}'..."));

        if !path.is_dir() {
            log!((logger.error) Backup("Archive does not exist, run `diary-cli init` to create a new one before you can back it up.") as Fatal);
            return logger.crash();
        }

        let database = if_err!((logger) [Backup, err => ("While backing up archive: {err:?}")] retry LazyDB::load_dir(&path));
        if_err!((logger) [Backup, err => ("While backing up archive: {err:?}")] retry database.compile(out_path));
        log!((logger.vital) Backup("Successfully backed up archive '{path_string}' as '{out_string}'") as Log);
        log!((logger) Backup(""));
    }

    /// Loads a backup if that backup is the same as the active archive and or newer than the active archive, otherwise errors will be thrown
    pub fn load_backup(path: impl AsRef<Path>, force: bool, mut logger: impl Logger) {
        let path = path.as_ref();
        let archive = home_dir().join("archive");
        let archive_string = archive.to_string_lossy();
        let path_string = path.to_string_lossy();

        log!((logger) Backup("Loading archive backup '{path_string}'..."));

        // Check if backup exists
        if !path.is_file() {
            log!((logger.error) Backup("Backup file '{path_string}' does not exist") as Fatal);
            return logger.crash();
        }

        // Check if archive already exists
        if archive.is_dir() {
            log!((logger.vital) Backup("Detected that there is already a loaded archive at '{archive_string}'") as Inconvenience);
            let old = Archive::load(logger.hollow()); // Loads old archive

            if force {
                log!((logger.vital) Backup("Forcefully loading backup; this may result in archive data loss") as Warning);
            }

            // Load new archive
            let new = home_dir().join("new");
            if_err!((logger) [Backup, err => ("While decompiling backup '{path_string}': {err:?}")] retry LazyDB::decompile(path, &new));
            let new = Archive::load_dir(new, logger.hollow());

            let _ = std::fs::remove_dir_all(new.database.path()); // cleanup

            // Check if uid is the same and that the itver is higher
            if new.uid != old.uid && !force {
                log!((logger.error) Backup("Cannot load backup as it is a backup of a different archive (uids don't match)") as Fatal);
                log!((logger.vital) Backup("If you still want to load it (deleting your current archive in the process) then run the same command but with `-f` to force it.") as Warning);
                return logger.crash();
            }

            if old.itver == new.itver && !force {
                log!((logger.vital) Backup("Detected that backup is the same age as the currently loaded archive (itver is the same)") as Warning);
            }

            if old.itver > new.itver && !force {
                log!((logger.error) Backup("Cannot load backup as it is older than the currently loaded archive (itver is less)") as Fatal);
                log!((logger.vital) Backup("If you still want to load it (losing un-backed changes in the process) then run the same command but with `-f` to force it.") as Warning);
                return logger.crash();
            }
            
            let _ = std::fs::remove_dir_all(&archive); // cleanup
        }

        if_err!((logger) [Backup, err => ("While decompiling backup '{path_string}': {err:?}")] retry LazyDB::decompile(path, &archive));
        log!((logger.vital) Backup("Successfully loaded backup '{path_string}'") as Log);
    }

    /// Wipes the specified archive and asks the user for confirmation
    pub fn wipe(self, mut logger: impl Logger) {
        // Confirm with the user about the action
        let expected = "I, as the user, confirm that I fully understand that I am wiping my ENTIRE archive and that this action is permanent and irreversible";
        log!((logger.vital) Wipe("To confirm with wiping your ENTIRE archive PERMANENTLY enter the phrase below (without quotes):") as Log);
        if_err!((logger) [Wipe, err => ("Entered phrase incorrect, please retry")] retry {
            log!((logger.vital) Wipe("\"{expected}\"") as Log);
            let input = logger.ask("Wipe", "Enter the phrase");
            if &input[0..input.len() - 1] != expected { Err(()) }
            else { Ok(()) }
        });

        log!((logger) Wipe("Wiping archive..."));

        let path = home_dir().join("archive");
        // Check if path exists
        if !path.exists() {
            log!((logger.vital) Wipe("Archive '{}' doesn't exist; doing nothing", path.to_string_lossy()) as Inconvenience);
            return;
        }

        // Wipe archive
        if_err!((logger) [Wipe, err => ("While wiping archive: {err:?}")] retry std::fs::remove_dir_all(&path));
        log!((logger.vital) Wipe("Successfully wiped archive! Run `diary-cli init` to init a new archive\n") as Log);
    }

    pub fn commit(&self, config: impl AsRef<Path>, mut logger: impl Logger) {
        let config = config.as_ref();
        let path = home_dir().join("archive");
        let path_string = path.to_string_lossy();

        // Checks if path exists or not
        if !path.is_dir() {
            log!((logger.error) Commit("Archive '{path_string}' doesn't exist! Run `diary-cli init` before you can commit") as Fatal);
            return logger.crash();
        }

        // Check if entry path exists or not
        let config_string = config.to_string_lossy();
        if !config.is_file() {
            log!((logger.error) Commit("Entry config file '{config_string}' doesn't exist") as Fatal);
            return logger.crash();
        }
        
        // Backup archive before modification
        let _ = std::fs::remove_file(home_dir().join("backup.ldb")); // Clean up
        Self::backup(home_dir().join("backup.ldb"), logger.hollow());

        // Parse toml
        log!((logger) Commit("Parsing toml at '{}'", config.to_string_lossy()));
        let entry = if_err!((logger) [Commit, err => ("While reading the entry config file: {err:?}")] retry std::fs::read_to_string(config));
        let entry = if_err!((logger) [Commit, err => ("While parsing entry config toml: {err:?}")] {entry.parse::<toml::Table>()} crash {
            log!((logger.error) Commit("{err:#?}") as Fatal);
            logger.crash()
        });

        
        // Checks if it is a moc
        let is_moc = entry.get("is-moc")
            .map(|x| unwrap_opt!((x.as_bool()) with logger, format: Commit("`is-moc` attribute of config file '{config_string}' must be boolean")))
            .unwrap_or(false);
        
        if is_moc {
            let container = if_err!((logger) [Commit, err => ("While loading archive as container: {err:?}")] retry search_database!((self.database) /mocs/));
            log!((logger) Commit("Detected that config file '{config_string}' is an moc (map of contents)"));
            MOC::new(entry, &config_string, container, logger.hollow());
        } else {
            let container = if_err!((logger) [Commit, err => ("While loading archive as container: {err:?}")] retry search_database!((self.database) /entries/));
            log!((logger) Commit("Detected that config file '{config_string}' is an entry"));
            
            // Add to unsorted list
            let entry = Entry::new(entry, &config_string, container, logger.hollow());
            log!((logger) Commit("Adding entry to unsorted stack..."));
            list::push(
                |file| LazyData::new_string(file, &entry.uid),
                &if_err!((logger) [Commit, err => ("While loaded unsorted stack: {err:?}")] retry search_database!((self.database) /order/unsorted)),
                logger.hollow(),
            );
        }

        // Update itver
        log!((logger) Commit("Updating archive itver..."));
        if_err!((logger) [Commit, err => ("While update archive itver: {err:?}")] retry write_database!((self.database) itver = new_u16(self.itver + 1)));

        log!((logger.vital) Commit("Successfully commited config to archive") as Log);
    }

    #[inline]
    pub fn database(&self) -> &LazyDB {
        &self.database
    }

    #[inline]
    pub fn database_exists(&self, path: impl AsRef<Path>) -> bool {
        self.database().path().join(path).exists()
    }

    pub fn get_entry(&self, uid: String, mut logger: impl Logger) -> Option<Entry> {
        if !self.database_exists(format!("entries/{uid}")) {
            log!((logger.error) Archive("Entry of uid `{uid}` does not exist") as Fatal);
            return logger.crash();
        }

        match search_database!((self.database) /entries/(&uid)) {
            Ok(x) => Some(Entry::load_lazy(uid, x)),
            Err(err) => match err {
                LDBError::DirNotFound(..) => None,
                _ => {
                    log!((logger.error) Archive("While getting entry '{uid}': {err:?}") as Fatal);
                    logger.crash()
                }
            }
        }
    }

    pub fn get_moc(&self, uid: String, mut logger: impl Logger) -> Option<MOC> {
        if !self.database_exists(format!("mocs/{uid}")) {
            log!((logger.error) Archive("Moc of uid `{uid}` does not exist") as Fatal);
            return logger.crash();
        }

        match search_database!((self.database) /mocs/(&uid)) {
            Ok(x) => Some(MOC::load_lazy(uid, x)),
            Err(err) => match err {
                LDBError::DirNotFound(..) => None,
                _ => {
                    log!((logger.error) Archive("While getting moc '{uid}': {err:?}") as Fatal);
                    logger.crash()
                }
            }
        }
    }

    pub fn list_entries(&self, mut logger: impl Logger) -> Vec<Entry> {
        let path = self.database.path().join("entries");

        if !path.is_dir() {
            log!((logger.vital) Entries("Path '{}' does not exist; doing nothing", path.to_string_lossy()) as Inconvenience);
            return Vec::with_capacity(0);
        }

        let mut logger1 = logger.hollow();
        let logger2 = logger.hollow();
        let dir = if_err!((logger) [Entries, err => ("While reading directory {}'s contents: {err:?}", path.to_string_lossy())] retry fs::read_dir(&path));
        dir.into_iter()
            .map(|x| if_err!((logger) [Entries, err => ("While reading dir element: {err:?}")] {x} crash logger.crash()))
            .filter(|x| if_err!((logger1) [Entries, err => ("While reading dir element: {err:?}")] {x.file_type()} crash logger1.crash()).is_dir())
            .map(|x| self.get_entry(x.file_name().to_string_lossy().to_string(), logger2.hollow()).unwrap())
            .collect()
    }

    pub fn list_mocs(&self, mut logger: impl Logger) -> Vec<MOC> {
        let path = self.database.path().join("mocs");

        if !path.is_dir() {
            log!((logger.vital) MOCs("Path '{}' does not exist; doing nothing", path.to_string_lossy()) as Inconvenience);
            return Vec::with_capacity(0);
        }

        let mut logger1 = logger.hollow();
        let logger2 = logger.hollow();
        let dir = if_err!((logger) [MOCs, err => ("While reading directory {}'s contents: {err:?}", path.to_string_lossy())] retry fs::read_dir(&path));
        dir.into_iter()
            .map(|x| if_err!((logger) [MOCs, err => ("While reading dir element: {err:?}")] {x} crash logger.crash()))
            .filter(|x| if_err!((logger1) [MOCs, err => ("While reading dir element: {err:?}")] {x.file_type()} crash logger1.crash()).is_dir())
            .map(|x| self.get_moc(x.file_name().to_string_lossy().to_string(), logger2.hollow()).unwrap())
            .collect()
    }
}