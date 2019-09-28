use std::env;
use std::io::BufWriter;
use std::ops::Range;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use cdb_rs::*;

fn parent_dir(pb: &PathBuf) -> Result<PathBuf> {
    if pb.is_relative() {
        let mut cwd = env::current_dir()?;
        cwd.push(pb);
        return parent_dir(&cwd);
    }

    Ok(pb.parent().unwrap().to_path_buf())
}

const ASCII: Range<u8> = 32u8..127u8;

pub fn create(path: &PathBuf) -> Result<()> {
    let mut bw = BufWriter::new(NamedTempFile::new_in(parent_dir(path)?)?);

    {
        let mut w = Writer::new(&mut bw)?;

        for x in ASCII {
            let k = [x];
            let v = [x];
            w.put(&k, &v)?;

            for y in ASCII {
                let k = [x, y];
                let v = [x, y];
                w.put(&k, &v)?;
            }
        }
    }

    let tf = bw.into_inner()?;
    tf.as_file().sync_all()?;
    tf.persist(path).map(|_| ()).map_err(|e| e.into())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use tempfile;

    #[test]
    fn create_and_read() {
        let tempdir = tempfile::TempDir::new().unwrap();

        let mut pb = tempdir.path().to_path_buf();
        pb.push("db.cdb");
        create(&pb).unwrap();

        let data = {
            let mut f = File::open(pb).unwrap();
            assert!(f.metadata().unwrap().len() > 2048);
            let mut b = Vec::new();

            let sz = f.read_to_end(&mut b).unwrap();
            assert_eq!(f.metadata().unwrap().len() as usize, sz);
            b.into_boxed_slice()
        };

        let reader = Reader::new(&data);

        for x in ASCII {
            let mut buf = vec![0u8; 2];
            let key = vec![x, x];

            let sz = reader.get(&key, &mut buf[..]).unwrap().unwrap();
            assert_eq!(sz, key.len());
            assert_eq!(&buf[0..key.len()], &key[..]);
        }
    }
}
