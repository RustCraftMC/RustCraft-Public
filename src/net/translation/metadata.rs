//! Entity metadata shape conversion between the 1.8 and 1.9 serializers.

use super::ProtocolError;

pub fn v47_to_v107(entries: &mut serde_json::Value) -> Result<(), ProtocolError> {
    let entries = entries
        .as_array_mut()
        .ok_or_else(|| ProtocolError("entity metadata must be an array".into()))?;
    for entry in entries {
        let object = entry
            .as_object_mut()
            .ok_or_else(|| ProtocolError("metadata entry must be an object".into()))?;
        let legacy_type = object
            .get("type")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ProtocolError("metadata entry requires numeric 'type'".into()))?;
        let modern_type = match legacy_type {
            0 => 0,     // byte
            1 | 2 => 1, // short/int → varint
            3 => 2,     // float
            4 => 3,     // string
            5 => 5,     // slot
            6 => 8,     // packed position
            7 => 7,     // rotation vector
            other => return Err(ProtocolError(format!("unknown v47 metadata type {other}"))),
        };
        object.insert("type".into(), serde_json::Value::from(modern_type));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_legacy_metadata_serializer_ids() {
        let mut metadata = serde_json::json!([
            {"index":0,"type":0,"value":1},
            {"index":1,"type":2,"value":42},
            {"index":2,"type":6,"value":123}
        ]);
        v47_to_v107(&mut metadata).unwrap();
        assert_eq!(metadata[1]["type"], 1);
        assert_eq!(metadata[2]["type"], 8);
    }
}
