use lazy_db::*;
use soulog::*;

pub fn write<T>(list: &[T], f: impl Fn(FileWrapper, &T) -> Result<(), LDBError>, container: &LazyContainer, mut logger: impl Logger) {
    for (i, x) in list.iter().enumerate() {
        let data_writer = 
            if_err!((logger) [ListIO, err => ("While writing element of list: {:?}", err)] retry container.data_writer(i.to_string()));
        if_err!((logger) [ListIO, err => ("While writing element of list: {:?}", err)] {f(data_writer, x)} crash {
            log!((logger.error) ListIO("{err:#?}") as Fatal);
            logger.crash()
        })
    }

    if_err!((logger) [ListIO, err => ("{:?}", err)] retry {
        let data_writer = if_err!((logger) [ListIO, err => ("While writing list length: {:?}", err)] retry container.data_writer("length"));
        LazyData::new_u16(data_writer, list.len() as u16)
    })
}

pub fn push(f: impl Fn(FileWrapper) -> Result<(), LDBError>, container: &LazyContainer, mut logger: impl Logger) {
    let length = if_err!((logger) [ListIO, err => ("While reading list legnth: {:?}", err)] retry container.read_data("length"));
    let length = if_err!((logger) [ListIO, err => ("While reading list length: {:?}", err)] {length.collect_u16()} crash {
        log!((logger.error) ListIO("{err:#?}") as Fatal);
        logger.crash()
    });

    let data_writer = if_err!((logger) [ListIO, err => ("While pushing to list: {:?}", err)] retry container.data_writer(length.to_string()));
    if_err!((logger) [ListIO, err => ("While pushing to list: {:?}", err)] {f(data_writer)} crash {
        log!((logger.error) ListIO("{err:#?}") as Fatal);
        logger.crash()
    });

    if_err!((logger) [ListIO, err => ("{:?}", err)] retry {
        let data_writer = if_err!((logger) [ListIO, err => ("While writing list length: {:?}", err)] retry container.data_writer("length"));
        LazyData::new_u16(data_writer, length + 1)
    })
}

pub fn read<T>(f: impl Fn(LazyData) -> Result<T, LDBError>, container: &LazyContainer, mut logger: impl Logger) -> Box<[T]> {
    let length = if_err!((logger) [ListIO, err => ("While reading list length: {:?}", err)] retry container.read_data("length"));
    let length = if_err!((logger) [ListIO, err => ("While reading list length: {:?}", err)] {length.collect_u16()} crash {
        log!((logger.error) ListIO("{err:#?}") as Fatal);
        logger.crash()
    }) as usize;

    let mut list = Vec::<T>::with_capacity(length);

    for i in 0..length {
        let item = if_err!((logger) [ListIO, err => ("While reading list element: {:?}", err)] retry container.read_data(i.to_string()));
        let item = if_err!((logger) [ListIO, err => ("While reading list element: {:?}", err)] {f(item)} crash {
            log!((logger.error) ListIO("{err:#?}") as Fatal);
            logger.crash()
        });
        list.push(item)
    }

    list.into_boxed_slice()
}