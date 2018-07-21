use super::KV;
use super::errors::CDBError;
use bytes::*;
use std::io::BufReader;
use std::io::prelude::*;
use std::str;

struct KVSizes(usize, usize);

const PLUS: u8 = 0x2b; // ASCII '+'
const COMMA: u8 = 0x2c; // ASCII ','
const COLON: u8 = 0x3a; // ASCII ':'
const NL: u8 = 0x0a; // ASCII '\n'

fn parse_digits(buf: &[u8]) -> Result<usize, CDBError> {
    str::from_utf8(&buf)
        .map_err(CDBError::UTF8Error)
        .and_then(|str| {
            str.parse::<usize>()
                .map_err(CDBError::ParseError)
        })
}

const ARROW_BYTES: &[u8; 2] = b"->";

// format: +1,3:a->xyz\n
fn read_begin<T: Read>(input: &mut BufReader<T>) -> Result<Option<()>, CDBError> {
    let mut buf = vec![0u8; 1];

    // consume a '+'
    input.read_exact(&mut buf)?;
    eprintln!("read_begin: {:?}", buf[0] as char);

    match buf[0] {
        PLUS => Ok(Some(())),
        NL => Ok(None),
        wat => panic!("encountered unexpected char: {:?}", wat as char),
    }
}

fn read_sizes<T: Read>(input: &mut BufReader<T>) -> Result<KVSizes, CDBError> {
    let mut buf: Vec<u8> = Vec::new();

    let r = input.read_until(COMMA, &mut buf)?;

    assert!(r > 0);
    assert_eq!(COMMA, buf.pop().unwrap());

    let k = parse_digits(&buf).unwrap();
    buf.clear();

    let r = input.read_until(COLON, &mut buf)?;

    assert!(r > 0);
    assert_eq!(COLON, buf.pop().unwrap());
    let v = parse_digits(&buf)?;

    Ok(KVSizes(k, v))
}

fn read_kv<T: Read>(input: &mut BufReader<T>, kvs: &KVSizes) -> Result<KV, CDBError> {
    let KVSizes(ksize, vsize) = kvs;

    let mut kbytes = vec![0u8; *ksize];
    input.read_exact(&mut kbytes)?;

    eprintln!("read K: {:?}", String::from_utf8(kbytes.clone()).unwrap());

    // consume the "->" between k and v
    let mut arrowbytes: [u8; 2] = [0; 2];
    input.read_exact(&mut arrowbytes)?;
    assert_eq!(arrowbytes, *ARROW_BYTES);

    let mut vbytes = vec![0u8; *vsize];
    input.read_exact(&mut vbytes)?;

    eprintln!("read V: {:?}", String::from_utf8(vbytes.clone()).unwrap());

    let mut newline = vec![0u8; 1];
    input.read_exact(&mut newline)?;

    assert_eq!(newline.len(), 1);
    assert_eq!(newline[0], NL);

    Ok(KV {
        k: Bytes::from(kbytes),
        v: Bytes::from(vbytes),
    })
}

fn read_one_record<T: Read>(input: &mut BufReader<T>) -> Result<Option<KV>, CDBError> {
    match read_begin(input)? {
        None => Ok(None),
        Some(_) => read_sizes(input)
            .and_then(|sizes| read_kv(input, &sizes))
            .map(Some),
    }
}

pub struct IterParser<T: Read> {
    buf: BufReader<T>,
}

impl<T: Read> Iterator for IterParser<T> {
    type Item = Result<KV, CDBError>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        match read_one_record(&mut self.buf) {
            Ok(Some(kv)) => Some(Ok(kv)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

// expects input in CDB format '+ks,vs:k->v\n'
pub fn parse<T: 'static + Read>(rdr: T) -> IterParser<T> {
    IterParser {
        buf: BufReader::new(rdr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_iter() {
        let reader = Bytes::from("+3,4:cat->ball\n\n").into_buf().reader();
        let recs: Vec<Result<KV, CDBError>> = parse(reader).collect();

        assert_eq!(recs.len(), 1);
        match recs[0] {
            Ok(KV { ref k, ref v }) => {
                assert_eq!(k, "cat");
                assert_eq!(v, "ball");
            }
            Err(ref x) => panic!("should not have errored: {:?}", x),
        };
    }

    #[test]
    fn parser_read_one_record() {
        let reader = Bytes::from("+3,4:cat->ball\n\n").into_buf().reader();
        let one = read_one_record(&mut BufReader::new(reader));

        match one {
            Ok(Some(KV { ref k, ref v })) => {
                assert_eq!(k, "cat");
                assert_eq!(v, "ball");
            }
            Ok(None) => panic!("got None expected Some"),
            Err(ref x) => panic!("should not have errored: {:?}", x),
        }
    }
}
