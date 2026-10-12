//! Penulisan dan pembacaan header DBF (dBASE III) untuk atribut SHP.
//! Hanya tipe Character (C) dan Numeric (N). Encoding ditulis UTF-8 melalui berkas .cpg.

use thiserror::Error;

const HEADER_BASE_LEN: usize = 32;
const FIELD_DESC_LEN: usize = 32;
const HEADER_TERMINATOR: u8 = 0x0D;
const EOF_MARKER: u8 = 0x1A;
/// Batas nama field dBASE. Nilai ini mengikuti aturan praktis Polify (lihat README).
pub const MAX_FIELD_NAME_LEN: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Character,
    Numeric { decimals: u8 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbfField {
    pub name: String,
    pub kind: FieldKind,
    pub length: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DbfValue {
    Text(String),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DbfInfo {
    pub record_count: u32,
    pub fields: Vec<DbfField>,
}

#[derive(Debug, Error, PartialEq)]
pub enum DbfError {
    #[error("nama field '{0}' harus 1–10 karakter A–Z, 0–9, atau '_'")]
    BadFieldName(String),
    #[error("baris {0} tidak memiliki jumlah nilai yang sesuai dengan field")]
    ColumnCount(usize),
    #[error("nilai field '{0}' tidak sesuai tipenya")]
    TypeMismatch(String),
    #[error("nilai teks pada field '{0}' melebihi {1} byte")]
    TextTooLong(String, u8),
    #[error("nilai angka pada field '{0}' tidak muat dalam {1} karakter")]
    NumberOverflow(String, u8),
    #[error("berkas DBF tidak valid: {0}")]
    Invalid(String),
}

/// Tulis DBF dari definisi field dan baris nilai. Tanggal pembaruan dibiarkan nol.
pub fn write_dbf(fields: &[DbfField], rows: &[Vec<DbfValue>]) -> Result<Vec<u8>, DbfError> {
    for f in fields {
        let valid_chars = f
            .name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if f.name.is_empty() || f.name.len() > MAX_FIELD_NAME_LEN || !valid_chars {
            return Err(DbfError::BadFieldName(f.name.clone()));
        }
    }

    let header_len = HEADER_BASE_LEN + FIELD_DESC_LEN * fields.len() + 1;
    let record_len = 1 + fields.iter().map(|f| f.length as usize).sum::<usize>();
    let mut out = Vec::with_capacity(header_len + rows.len() * record_len + 1);

    out.push(0x03);
    out.extend_from_slice(&[0, 0, 0]);
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&(header_len as u16).to_le_bytes());
    out.extend_from_slice(&(record_len as u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);

    for f in fields {
        let mut name = [0u8; 11];
        name[..f.name.len()].copy_from_slice(f.name.as_bytes());
        out.extend_from_slice(&name);
        out.push(match f.kind {
            FieldKind::Character => b'C',
            FieldKind::Numeric { .. } => b'N',
        });
        out.extend_from_slice(&[0u8; 4]);
        out.push(f.length);
        out.push(match f.kind {
            FieldKind::Numeric { decimals } => decimals,
            FieldKind::Character => 0,
        });
        out.extend_from_slice(&[0u8; 14]);
    }
    out.push(HEADER_TERMINATOR);

    for (row_idx, row) in rows.iter().enumerate() {
        if row.len() != fields.len() {
            return Err(DbfError::ColumnCount(row_idx + 1));
        }
        out.push(b' ');
        for (field, value) in fields.iter().zip(row) {
            out.extend_from_slice(&encode_cell(field, value)?);
        }
    }
    out.push(EOF_MARKER);
    Ok(out)
}

fn encode_cell(field: &DbfField, value: &DbfValue) -> Result<Vec<u8>, DbfError> {
    let len = field.length as usize;
    match (&field.kind, value) {
        (FieldKind::Character, DbfValue::Text(text)) => {
            if text.len() > len {
                return Err(DbfError::TextTooLong(field.name.clone(), field.length));
            }
            let mut bytes = text.as_bytes().to_vec();
            bytes.resize(len, b' ');
            Ok(bytes)
        }
        (FieldKind::Numeric { decimals }, DbfValue::Number(x)) => {
            if !x.is_finite() {
                return Err(DbfError::TypeMismatch(field.name.clone()));
            }
            let text = format!("{:.*}", *decimals as usize, x);
            if text.len() > len {
                return Err(DbfError::NumberOverflow(field.name.clone(), field.length));
            }
            let mut bytes = vec![b' '; len - text.len()];
            bytes.extend_from_slice(text.as_bytes());
            Ok(bytes)
        }
        _ => Err(DbfError::TypeMismatch(field.name.clone())),
    }
}

/// Baca header DBF: jumlah record, daftar field, dan memastikan data tidak terpotong.
pub fn read_header(bytes: &[u8]) -> Result<DbfInfo, DbfError> {
    if bytes.len() < HEADER_BASE_LEN {
        return Err(DbfError::Invalid("header kurang dari 32 byte".into()));
    }
    let record_count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as u64;
    let record_len = u16::from_le_bytes([bytes[10], bytes[11]]) as u64;
    if (bytes.len() as u64) < header_len + record_count as u64 * record_len {
        return Err(DbfError::Invalid("data record terpotong".into()));
    }

    let mut fields = Vec::new();
    let mut pos = HEADER_BASE_LEN;
    while (pos as u64) < header_len && bytes.get(pos) != Some(&HEADER_TERMINATOR) {
        let desc = bytes
            .get(pos..pos + FIELD_DESC_LEN)
            .ok_or_else(|| DbfError::Invalid("deskripsi field terpotong".into()))?;
        let name: String = desc[..11]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        let kind = match desc[11] {
            b'C' => FieldKind::Character,
            b'N' => FieldKind::Numeric { decimals: desc[17] },
            other => {
                return Err(DbfError::Invalid(format!(
                    "tipe field '{}' belum didukung",
                    other as char
                )))
            }
        };
        fields.push(DbfField {
            name,
            kind,
            length: desc[16],
        });
        pos += FIELD_DESC_LEN;
    }

    Ok(DbfInfo {
        record_count,
        fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<DbfField> {
        vec![
            DbfField {
                name: "NAMA".into(),
                kind: FieldKind::Character,
                length: 20,
            },
            DbfField {
                name: "LUAS_HA".into(),
                kind: FieldKind::Numeric { decimals: 4 },
                length: 12,
            },
        ]
    }

    #[test]
    fn writes_and_reads_back_header() {
        let rows = vec![
            vec![DbfValue::Text("Warung Ilham".into()), DbfValue::Number(12.5)],
            vec![DbfValue::Text("Kantor Ö".into()), DbfValue::Number(0.0421)],
        ];
        let bytes = write_dbf(&fields(), &rows).unwrap();
        let info = read_header(&bytes).unwrap();
        assert_eq!(info.record_count, 2);
        let names: Vec<_> = info.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["NAMA", "LUAS_HA"]);
        // header 32 + 2 * 32 + 1 terminator; record 1 + 20 + 12; EOF marker di akhir
        assert_eq!(bytes.len(), 97 + 2 * 33 + 1);
        assert_eq!(*bytes.last().unwrap(), EOF_MARKER);
    }

    #[test]
    fn rejects_long_names_bad_types_and_overflow() {
        let long = vec![DbfField {
            name: "NAMA_FIELD_TERLALU".into(),
            kind: FieldKind::Character,
            length: 5,
        }];
        assert!(matches!(write_dbf(&long, &[]), Err(DbfError::BadFieldName(_))));

        let too_long = "teks ini jauh lebih panjang dari dua puluh byte";
        let rows = vec![vec![DbfValue::Text(too_long.into()), DbfValue::Number(1.0)]];
        assert!(matches!(
            write_dbf(&fields(), &rows),
            Err(DbfError::TextTooLong(_, 20))
        ));

        let rows = vec![vec![DbfValue::Text("ok".into()), DbfValue::Number(1e12)]];
        assert!(matches!(
            write_dbf(&fields(), &rows),
            Err(DbfError::NumberOverflow(_, 12))
        ));
    }
}
