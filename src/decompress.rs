use std::io::{Read, Seek, Write};

type Error = Box<dyn std::error::Error>;

/// Deflate as a stream
pub fn decompress<R: Read, W: Read + Write + Seek>(
    mut reader: R,
    mut writer: W,
) -> Result<(), Error> {
    let mut dictionary = Vec::new();

    loop {
        match get_control_bytes(&mut reader) {
            Ok(offset) => {
                match offset {
                    Offset::Dictionary { length, offset } => {
                        let dict = fetch_offset(&dictionary, length, offset)?;
                        dictionary.extend_from_slice(&dict);
                    }
                    Offset::Literal { length } => match read_bytes(&mut reader, length) {
                        Ok(bytes) => {
                            dictionary.append(&mut bytes.to_vec());
                        }
                        Err(_) => {
                            return Err("Cannot take any more literal bytes, reached end of compressed buffer.".into());
                        }
                    },
                }
            }
            Err(_) => {
                break;
            }
        }
    }

    writer.write_all(&dictionary)?;
    Ok(())
}

/// Fetch a series of bytes from a the dictionary at a given offset
fn fetch_offset(dictionary: &[u8], length: usize, offset: usize) -> Result<Vec<u8>, Error> {
    if offset > dictionary.len() {
        return Err("Offset larger than dictionary".into());
    }

    let mut result = Vec::with_capacity(length);

    for i in 0..length {
        let pos = dictionary.len() - offset + (i % offset);
        if pos >= dictionary.len() {
            return Err("Index out of bounds.".into());
        }
        result.push(dictionary[pos]);
    }

    Ok(result)
}

#[derive(Debug, PartialEq)]
enum Offset {
    Literal { length: usize },
    Dictionary { length: usize, offset: usize },
}

/// Control bytes are used by the compression algorithm to determine what kind of data is in the next chunk.
fn get_control_bytes<R: Read>(reader: &mut R) -> Result<Offset, Error> {
    let cb = read_u8(reader)?;
    let q = q_mask(cb) as usize;
    let cb_mask = cb_mask(cb) as usize;

    Ok(match cb_mask {
        1 => Offset::Literal { length: 1 + q },
        3..=8 => {
            let r = read_u8(reader)?;
            Offset::Dictionary {
                length: cb_mask,
                offset: ((q << 8) + r as usize + 1),
            }
        }

        9 => {
            let r = read_u8(reader)?;
            let s = read_u8(reader)?;

            Offset::Dictionary {
                length: 9 + r as usize,
                offset: ((q << 8) + s as usize + 1),
            }
        }
        _ => unreachable!(),
    })
}

fn cb_mask(i: u8) -> u8 {
    if i | 0b0001_1111 == 0b0001_1111 {
        return 1;
    }

    if i | 0b0011_1111 == 0b0011_1111 {
        return 3;
    }

    if i | 0b0101_1111 == 0b0101_1111 {
        return 4;
    }

    if i | 0b0111_1111 == 0b0111_1111 {
        return 5;
    }

    if i | 0b1001_1111 == 0b1001_1111 {
        return 6;
    }

    if i | 0b1011_1111 == 0b1011_1111 {
        return 7;
    }

    if i | 0b1101_1111 == 0b1101_1111 {
        return 8;
    }

    if i | 0b1111_1111 == 0b1111_1111 {
        return 9;
    }

    panic!("Unknown control byte. [{:08b}:{:02X}]", i, i);
}

/// bitwise mask to determine 'Q'
fn q_mask(i: u8) -> u8 {
    i & 0b0001_1111
}

fn read_bytes(reader: &mut dyn Read, bytes: usize) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = vec![0u8; bytes];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_u8(reader: &mut dyn Read) -> Result<u8, std::io::Error> {
    let mut buf = vec![0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Cursor};

    use super::*;

    #[test]
    fn test_cb_mask() {
        assert_eq!(cb_mask(0b00000001), 1);
        assert_eq!(cb_mask(0b00100001), 3);
        assert_eq!(cb_mask(0b01000001), 4);
        assert_eq!(cb_mask(0b01100001), 5);
        assert_eq!(cb_mask(0b10000001), 6);
        assert_eq!(cb_mask(0b10100001), 7);
        assert_eq!(cb_mask(0b11000101), 8);
        assert_eq!(cb_mask(0b11100001), 9);
    }

    #[test]
    fn test_q_mask() {
        assert_eq!(q_mask(0b11100001), 1);
        assert_eq!(q_mask(0b11100010), 2);
        assert_eq!(q_mask(0b00000011), 3);
    }

    #[test]
    fn test_get_control_bytes() -> Result<(), Error> {
        use Offset::*;

        assert_eq!(
            get_control_bytes(&mut Cursor::new([0x02]))?,
            Literal { length: 3 }
        );

        assert_eq!(
            get_control_bytes(&mut Cursor::new([0x20, 0x0E]))?,
            Dictionary {
                length: 3,
                offset: 15
            }
        );

        assert_eq!(
            get_control_bytes(&mut Cursor::new([0x60, 0x00]))?,
            Dictionary {
                length: 5,
                offset: 1
            }
        );

        Ok(())
    }

    #[test]
    fn test_fetch_offset() {
        assert_eq!(
            fetch_offset(&vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07], 3, 7).unwrap(),
            vec![0x01, 0x02, 0x03]
        );

        assert_eq!(
            fetch_offset(&vec![0x01, 0x02, 0x03, 0xF4, 0x15, 0x06], 1, 5).unwrap(),
            vec![0x02]
        );

        assert_eq!(
            fetch_offset(&vec![0x00, 0x01, 0x00, 0x00, 0x00], 16, 4).unwrap(),
            vec![
                0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00
            ]
        );

        assert_eq!(
            fetch_offset(&vec![0x01, 0x02, 0xF4, 0x08, 0x00], 3, 1).unwrap(),
            vec![0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn test_deflate_file() -> Result<(), Error> {
        let input = File::open("tests/data/000.compressed")?;
        let mut output = Cursor::new(Vec::new());

        decompress(input, &mut output)?;

        Ok(assert_eq!(
            std::fs::read("tests/data/000.decompressed")?,
            output.into_inner(),
        ))
    }
}
